# Почему Rust + egui для LL-Control?

## Краткий ответ: **Rust + egui** - лучший выбор

---

## Сравнение вариантов

### 1. **Rust + egui** ⭐ (рекомендую)

```
✅ HID поддержка отличная (hidapi crate)
✅ Нативный быстрый GUI
✅ Маленький binary (~5MB)
✅ Лёгкая разработка (immediate mode GUI)
✅ Отлично для кастомных графиков кривых
✅ Без runtime зависимостей
✅ Cross-platform из коробки
```

**Пример кода (100 строк):**
```rust
use eframe::egui;

fn main() {
    eframe::run_native("L-Connect", Default::default(),
        Box::new(|cc| Box::new(MyApp::new(cc)))
    );
}

struct MyApp { fan_pwm: f32 }

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add(egui::Slider::new(&mut self.fan_pwm, 0.0..=100.0));
            if ui.button("Apply").clicked() {
                // Send PWM command to device
            }
        });
    }
}
```

**Когда выбрать:**
- ✅ Production приложение для Linux
- ✅ Нужна производительность
- ✅ Хотете лёгкую дистрибуцию
- ✅ Планируете поддержку других ОС

---

### 2. **Rust + Tauri** (для красивого Web UI)

```
✅ Можно использовать React/Vue/Svelte
✅ Красивый UI "из коробки"
✅ Отличный для сложных интерфейсов

❌ WebView оверхед (~50-100MB RAM)
❌ Сложнее сборка
❌ Большой binary (~100MB+)
```

**Когда выбрать:**
- Нужен очень сложный, современный UI
- У вас есть опыт с frontend разработкой
- Красивость важнее производительности

---

### 3. **Python + PyQt/PySide**

```
✅ Быстрый prototyping (часы вместо дней)
✅ Огромная ecosystem
✅ Легко интегрировать lm-sensors

❌ Нужен Python runtime
❌ Медленнее нативного
❌ Сложнее дистрибуция
```

**Когда выбрать:**
- Rapid prototyping / PoC
- Не критична производительность
- Удобство разработки важнее всего

---

### 4. **Go + Fyne**

```
✅ Простой язык
✅ Быстрая разработка
✅ Кросс-платформа

❌ GC pauses
❌ Хуже для low-level работы
❌ Меньше производительность
```

**Когда выбрать:**
- У вас уже есть опыт с Go
- Простота важнее производительности

---

## Почему не C# на Linux?

| Проблема | Почему это важно |
|----------|------------------|
| **Avalonia UI** | Меньше ecosystem чем у Rust |
| **Gtk#** | Устаревший, сложный API |
| **.NET runtime** | Нужен на target системе |
| **Startup time** | Медленнее чем Rust |
| **Binary size** | Большой размер |

**C# имеет смысл если:**
- ✅ Уже есть большой опыт с WPF
- ✅ Нужна совместимость с существующим кодом

---

## Мой выбор: Rust + egui

### Практические преимущества

#### 1. **Установка зависимостей**
```bash
# Ubuntu/Debian
sudo apt install libudev-dev pkg-config

# Fedora/RHEL
sudo dnf install systemd-devel

# Arch Linux
sudo pacman -S systemd
```

#### 2. **Сборка**
```bash
cargo build --release

# Получаете один static binary
./target/release/llcontrol_linux

# ~5MB, без зависимостей!
```

#### 3. **Дистрибуция**
```bash
# Просто скопируйте binary
cp target/release/llcontrol_linux /usr/local/bin/

# Или создайте DEB/RPM
cargo install cargo-deb
cargo deb
```

---

## Структура проекта

```
ll-control/
├── Cargo.toml                 # Зависимости
├── src/
│   ├── main.rs                # Entry point
│   ├── app.rs                 # GUI (egui)
│   ├── device/
│   │   ├── mod.rs             # Модуль устройств
│   │   ├── galahad.rs         # GALAHAD II protocol
│   │   └── commands.rs        # Command definitions
│   └── sensors/
│       ├── mod.rs             # Модуль датчиков
│       └── lm_sensors.rs      # lm-sensors wrapper
└── assets/
    └── icon.png               # App icon
```

---

## Ключевые крейты

| Крейт | Назначение |
|-------|------------|
| `eframe` | egui framework |
| `egui_plot` | Графики кривых |
| `hidapi` | HID коммуникация |
| `tokio` | Async runtime |
| `serde` | Сериализация |
| `anyhow` | Error handling |
| `tracing` | Логирование |

---

## Примеры кода

### HID подключение
```rust
use hidapi::HidApi;

let api = HidApi::new()?;
let device = api.open(VID, PID)?;

// Non-blocking режим
device.set_blocking_mode(false)?;

// Чтение handshake
let mut buf = [0u8; 64];
device.write(&[1, 129, 0, 0, 0, 0])?;  // Command 129
device.read_timeout(&mut buf, 1000)?;
```

### Fan control
```rust
// Command 139: SetFanPWM
let mut packet = [0u8; 64];
packet[0] = 1;  // Report ID
packet[1] = 139; // SetFanPWM
packet[2] = percentage; // 0-100

device.write(&packet)?;
```

### lm-sensors (через sysfs)
```rust
// Читать температуру CPU
let temp = std::fs::read_to_string(
    "/sys/class/thermal/thermal_zone0/temp"
)?;

let temp_c: f32 = temp.trim().parse::<u32>()? as f32 / 1000.0;
```

---

## Roadmap

### Phase 1: MVP (1-2 недели)
- [x] Базовый GUI с egui
- [x] HID подключение к устройству
- [x] Чтение handshake (температура, RPM)
- [x] Управление fan/pump PWM
- [ ] Базовый fan curve editor

### Phase 2: Production (2-4 недели)
- [ ] lm-sensors интеграция
- [ ] Fan curve с drag-and-drop
- [ ] Профили (сохранение/загрузка)
- [ ] System tray icon
- [ ] Автостарт при login

### Phase 3: Advanced (1-2 месяца)
- [ ] LCD image display
- [ ] JPEG upload
- [ ] LED color control
- [ ] H264 video streaming
- [ ] Theme editor

---

## Альтернативы (если хотите попробовать)

### Python прототип (за 1 час)
```bash
pip install hidapi pyqt6

# 100 строк кода для базового control
# Отлично для быстрого тестирования протокола
```

Затем **переписать на Rust** для production.

---

## Итог

| Для чего | Используйте |
|----------|-------------|
| **Production** | Rust + egui |
| **Быстрый PoC** | Python + PyQt |
| **Красивый Web UI** | Rust + Tauri |
| **Mobile/portable** | Rust + Flutter (embedded) |

**Рекомендация:** Начните с **Rust + egui**.

Этот boilerplate проект уже создан в `ll-control/`!

```bash
cd ll-control
cargo build --release
./target/release/llcontrol_linux
```

Good luck! 🦀
