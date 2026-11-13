// SPDX-FileCopyrightText: 2026 Pavel Obruchnikov <me@inkyquill.net>
// SPDX-License-Identifier: GPL-3.0-or-later
#include "feed.h"
#include <QJsonDocument>
#include <QJsonObject>
#include <QStandardPaths>
#include <limits>

static constexpr qsizetype maxLine=1024*1024;
Feed::Feed(QObject *parent):QObject(parent) {
    m_retry.setSingleShot(true);m_retry.setInterval(1000);
    connect(&m_retry,&QTimer::timeout,this,&Feed::start);
    m_clock.setInterval(250);
    connect(&m_clock,&QTimer::timeout,this,[this]{
        if(m_received.isValid()&&m_received.elapsed()>5000)unavailable();
        Q_EMIT elapsedChanged();
    });
    connect(&m_process,&QProcess::readyReadStandardOutput,this,&Feed::read);
    // Drain diagnostics separately; they never become sensor values or rich text.
    connect(&m_process,&QProcess::readyReadStandardError,this,[this]{m_process.readAllStandardError();});
    connect(&m_process,&QProcess::errorOccurred,this,[this]{failed();});
    connect(&m_process,qOverload<int,QProcess::ExitStatus>(&QProcess::finished),this,[this]{failed();});
}
Feed::~Feed(){m_active=false;stop();}
int Feed::elapsedMs() const {return m_received.isValid()?int(qMin<qint64>(m_received.elapsed(),std::numeric_limits<int>::max())):std::numeric_limits<int>::max();}
void Feed::setActive(bool active) {
    if(m_active==active)return;
    m_active=active;Q_EMIT activeChanged();
    if(active){m_clock.start();start();}else stop();
}
void Feed::stop() {
    m_retry.stop();m_clock.stop();
    if(m_process.state()!=QProcess::NotRunning){m_process.kill();m_process.waitForFinished(1000);}
    m_buffer.clear();unavailable();
}
void Feed::start() {
    if(!m_active||m_process.state()!=QProcess::NotRunning)return;
    m_buffer.clear();
    const QString executable=QStandardPaths::findExecutable(QStringLiteral("llcontrol"));
    if(executable.isEmpty()){failed();return;}
    // Fixed executable/arguments; no QML-provided commands, shell or paths.
    m_process.start(executable,{QStringLiteral("mini"),QStringLiteral("watch")});
}
void Feed::unavailable() {
    m_received.invalidate();
    m_frame={{QStringLiteral("version"),1},{QStringLiteral("available"),false}};
    m_frameJson=QStringLiteral("{\"version\":1,\"available\":false}");
    Q_EMIT frameChanged();Q_EMIT elapsedChanged();
}
void Feed::failed() {
    unavailable();m_buffer.clear();
    if(m_active&&!m_retry.isActive())m_retry.start();
}
void Feed::read() {
    while(m_process.bytesAvailable()>0) {
        const QByteArray bytes=m_process.read(qMin<qint64>(m_process.bytesAvailable(),65536));
        if(bytes.isEmpty())break;
        m_buffer+=bytes;
        while(true) {
            const qsizetype newline=m_buffer.indexOf('\n');
            if(newline<0)break;
            if(newline>maxLine){m_process.kill();failed();return;}
            const QByteArray line=m_buffer.left(newline);m_buffer.remove(0,newline+1);
            QJsonParseError error;
            const auto document=QJsonDocument::fromJson(line,&error);
            const auto object=document.object();
            if(error.error!=QJsonParseError::NoError||!document.isObject()||object.value("version").toInt(-1)!=1||!object.value("available").isBool()||
               (object.value("available").toBool()&&!object.value("data").isObject())) {
                m_process.kill();failed();return;
            }
            m_frame=object.toVariantMap();m_frameJson=QString::fromUtf8(line);m_received.restart();Q_EMIT frameChanged();Q_EMIT elapsedChanged();
        }
        if(m_buffer.size()>maxLine){m_process.kill();failed();return;}
    }
}
