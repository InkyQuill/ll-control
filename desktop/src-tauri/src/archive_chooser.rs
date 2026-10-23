//! A local file grant from XDG FileChooser. No filesystem path is accepted from
//! the webview. Dedicated portal connection and Request::Close own cancellation.
use anyhow::{ensure, Context, Result};
use futures_util::StreamExt;
use lconnect_ipc::InterfaceLanguage;
use std::{collections::HashMap, path::PathBuf, time::Duration};
use tokio::sync::watch;
use zbus::{
    zvariant::{OwnedObjectPath, OwnedValue, Value},
    Connection, Proxy,
};

#[derive(Clone, Copy)]
struct Dialog {
    save: bool,
    group: bool,
    windows: bool,
    language: InterfaceLanguage,
}
impl Dialog {
    fn text(self, english: &'static str, russian: &'static str) -> &'static str {
        match self.language {
            InterfaceLanguage::English => english,
            InterfaceLanguage::Russian => russian,
        }
    }
    fn title(self) -> &'static str {
        if self.windows {
            return self.text(
                "Import Windows L-Connect3 settings",
                "Импорт настроек Windows L-Connect3",
            );
        }
        match (self.language, self.group, self.save) {
            (InterfaceLanguage::English, false, true) => "Export LL-Control profile",
            (InterfaceLanguage::English, false, false) => "Import LL-Control profile",
            (InterfaceLanguage::English, true, true) => "Export LL-Control settings group",
            (InterfaceLanguage::English, true, false) => "Import LL-Control settings group",
            (InterfaceLanguage::Russian, false, true) => "Экспорт профиля LL-Control",
            (InterfaceLanguage::Russian, false, false) => "Импорт профиля LL-Control",
            (InterfaceLanguage::Russian, true, true) => "Экспорт группы настроек LL-Control",
            (InterfaceLanguage::Russian, true, false) => "Импорт группы настроек LL-Control",
        }
    }
    fn filter(self) -> &'static str {
        if self.windows {
            return "Windows L-Connect3 ZIP";
        }
        match (self.language, self.group) {
            (InterfaceLanguage::English, false) => "LL-Control profile",
            (InterfaceLanguage::English, true) => "LL-Control settings group",
            (InterfaceLanguage::Russian, false) => "Профиль LL-Control",
            (InterfaceLanguage::Russian, true) => "Группа настроек LL-Control",
        }
    }
}

pub async fn choose(
    save: bool,
    suggested_name: String,
    cancel: watch::Receiver<bool>,
    language: InterfaceLanguage,
) -> Result<Option<PathBuf>> {
    let connection = Connection::session().await?;
    choose_filtered(
        &connection,
        &suggested_name,
        cancel,
        Duration::from_secs(600),
        Dialog {
            save,
            group: false,
            windows: false,
            language,
        },
    )
    .await
}
pub async fn choose_group(
    save: bool,
    cancel: watch::Receiver<bool>,
    language: InterfaceLanguage,
) -> Result<Option<PathBuf>> {
    let connection = Connection::session().await?;
    choose_filtered(
        &connection,
        "backup.lcg",
        cancel,
        Duration::from_secs(600),
        Dialog {
            save,
            group: true,
            windows: false,
            language,
        },
    )
    .await
}
pub async fn choose_windows_group(
    cancel: watch::Receiver<bool>,
    language: InterfaceLanguage,
) -> Result<Option<PathBuf>> {
    let connection = Connection::session().await?;
    choose_filtered(
        &connection,
        "backup.zip",
        cancel,
        Duration::from_secs(600),
        Dialog {
            save: false,
            group: true,
            windows: true,
            language,
        },
    )
    .await
}
async fn choose_filtered(
    connection: &Connection,
    suggested_name: &str,
    mut cancel: watch::Receiver<bool>,
    deadline: Duration,
    dialog: Dialog,
) -> Result<Option<PathBuf>> {
    let Dialog { save, group, .. } = dialog;
    let token = format!("lconnect_archive_{}", uuid::Uuid::new_v4().simple());
    let sender = connection
        .unique_name()
        .context(dialog.text(
            "Missing file chooser identity",
            "Не удалось определить соединение диалога выбора файла",
        ))?
        .as_str()
        .trim_start_matches(':')
        .replace('.', "_");
    let path = format!("/org/freedesktop/portal/desktop/request/{sender}/{token}");
    let portal = Proxy::new(
        connection,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.FileChooser",
    )
    .await?;
    let request = Proxy::new(
        connection,
        "org.freedesktop.portal.Desktop",
        path.as_str(),
        "org.freedesktop.portal.Request",
    )
    .await?;
    let mut responses = request.receive_signal("Response").await?;
    let name = suggested_name
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '/' | '\\'))
        .take(80)
        .collect::<String>();
    let name = if name.trim().is_empty() {
        "profile.lcp"
    } else {
        &name
    };
    let filters = if dialog.windows {
        vec![(dialog.filter(), vec![(0u32, "*.zip")])]
    } else if group {
        vec![(dialog.filter(), vec![(0u32, "*.lcg")])]
    } else {
        vec![(dialog.filter(), vec![(0u32, "*.lcp")])]
    };
    let mut options = HashMap::from([
        ("handle_token", Value::from(token.as_str())),
        ("modal", Value::from(true)),
        ("filters", Value::from(filters)),
    ]);
    if save {
        options.insert("current_name", Value::from(name));
    } else {
        options.insert("multiple", Value::from(false));
        options.insert("directory", Value::from(false));
    }
    let mut cancelled = false;
    let outcome = tokio::time::timeout(deadline, async {
        if *cancel.borrow() {
            cancelled = true;
            return Ok(None);
        }
        let method = if save { "SaveFile" } else { "OpenFile" };
        let title = dialog.title();
        let handle: OwnedObjectPath = tokio::time::timeout(
            Duration::from_secs(10),
            portal.call(method, &("", title, options)),
        )
        .await.context(dialog.text("File chooser did not open in time", "Диалог выбора файла не открылся вовремя"))?
        .context(dialog.text("Could not open file chooser", "Не удалось открыть диалог выбора файла"))?;
        if handle.as_str() != path {
            if let Ok(actual) = Proxy::new(
                connection,
                "org.freedesktop.portal.Desktop",
                handle.as_str(),
                "org.freedesktop.portal.Request",
            )
            .await
            {
                let _ = tokio::time::timeout(
                    Duration::from_secs(2),
                    actual.call::<_, _, ()>("Close", &()),
                )
                .await;
            }
            anyhow::bail!("{}", dialog.text("File chooser returned an unexpected request handle", "Диалог выбора файла вернул неожиданный идентификатор запроса"));
        }
        let message = tokio::select! {
            biased;
            _=cancel.wait_for(|v|*v)=>{cancelled=true;return Ok(None);},
            message=responses.next()=>message.context(dialog.text("File chooser disconnected", "Соединение с диалогом выбора файла потеряно"))?,
        };
        let (code, values): (u32, HashMap<String, OwnedValue>) = message.body().deserialize()?;
        match code {
            0 => {
                let uris = Vec::<String>::try_from(
                    values
                        .get("uris")
                        .context(dialog.text("File chooser returned no files", "Диалог выбора файла не вернул файл"))?
                        .try_clone()?,
                )?;
                ensure!(uris.len() == 1, "{}", dialog.text("Choose exactly one archive file", "Выберите ровно один файл архива"));
                Ok(Some(local_path(&uris[0], dialog)?))
            }
            1 => Ok(None),
            _ => anyhow::bail!("{} ({code})", dialog.text("File chooser rejected operation", "Диалог выбора файла отклонил операцию")),
        }
    })
    .await;
    if cancelled || !matches!(&outcome, Ok(Ok(_))) {
        let _ = tokio::time::timeout(
            Duration::from_secs(2),
            request.call::<_, _, ()>("Close", &()),
        )
        .await;
    }
    outcome.context(dialog.text(
        "File selection timed out",
        "Время ожидания выбора файла истекло",
    ))?
}
fn local_path(uri: &str, dialog: Dialog) -> Result<PathBuf> {
    let uri = url::Url::parse(uri)
        .context(dialog.text("Invalid archive file URI", "Некорректный URI файла архива"))?;
    ensure!(
        uri.scheme() == "file"
            && uri.query().is_none()
            && uri.fragment().is_none()
            && uri.username().is_empty()
            && uri.password().is_none(),
        dialog.text(
            "File chooser must return a local file URI",
            "Диалог должен вернуть локальный файл архива"
        )
    );
    uri.to_file_path().map_err(|()| {
        anyhow::anyhow!(
            "{}",
            dialog.text(
                "Invalid local archive path",
                "Некорректный путь локального архива"
            )
        )
    })
}

#[cfg(test)]
#[path = "archive_chooser_tests.rs"]
mod tests;
