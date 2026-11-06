# Проверки этой версии

Проверено в облачной Linux-среде 6 ноября 2026 года. Это preview, не подтверждённый коммерческий релиз.

## Обновление 0.1.1

* `cargo test --workspace --locked --offline`: 9 тестов прошли; отдельный OpenSSH-тест запускается через `bash scripts/ssh-smoke.sh`.
* Настоящий локальный OpenSSH: вход root, вход ubuntu с паролем sudo, выполнение команды от UID 0, сохранность скрипта и пароля со спецсимволами, отказ пользователю без sudo и понятная ошибка нехватки RAM — успешно.
* SSH: первое автоматическое запоминание, повторное подключение, отклонение несовпадающего сохранённого ключа до передачи пароля — успешно.
* TypeScript/Vite, 3 Vitest-теста, браузерный мастер с четырьмя полями и тестовыми native-вызовами, Clippy, rustfmt и shell syntax — успешно.
* Нативный Tauri: Linux `cargo check` в контейнере — успешно. Windows x64 release EXE версии 0.1.1 с `custom-protocol` пересобран после исправления sudo; упакован вместе с WebView2Loader.dll.
* Проверки выполнялись на Linux и локальных контейнерах. Запуск EXE непосредственно на Windows и установка на внешнем VPS пользователя не выполнялись. ЮKassa в скачиваемой preview-сборке не настроена; платная установка остаётся недоступна.

## Базовые проверки 0.1.0

Ниже — результаты предыдущего прогона. Неизменённые VPN-протоколы повторно в 0.1.1 не испытывались.

| Проверка | Результат и границы |
| --- | --- |
| Rust workspace | `cargo build --workspace --locked` — успешно; ядро, CLI, billing |
| Rust tests | 8 тестов прошли: 5 профилей/валидации/лицензий/manifest, 1 настоящий локальный SSH-тест, 2 billing-теста |
| SSH | Проверены отказ до передачи пароля при чужом host key, правильный/неправильный пароль, поток вывода и ненулевой exit code |
| Платёжный API | Локальный тестовый HTTP-провайдер: pending→succeeded, ровно один create при повторе, чужой/отсутствующий capability, неверная сумма, подпись лицензии, остановка повтора после окна идемпотентности |
| Clippy и форматирование | Workspace Clippy с `-D warnings`, rustfmt — успешно |
| UI | TypeScript/Vite production build и 3 Vitest-теста — успешно |
| Браузер | Chromium, 1360 px и 390 px, без горизонтального переполнения; полный мастер с тестовым native-adapter; отдельная проверка блокировки SSH в обычном браузере |
| Tauri Linux | `cargo check` в Debian с GTK/WebKit — успешно; это не испытание мобильных API на устройствах |
| Windows | Release EXE x86_64 GNU с `custom-protocol` собран; PE-формат и DLL imports проверены. Рядом упаковывается WebView2Loader.dll. Запуск на Windows и цифровая подпись не проверены |
| Xray | Официальный `ghcr.io/xtls/xray-core:26.10.31` принял и серверный, и клиентский JSON через `run -test` |
| AmneziaWG 3.1 | Образ собран из зафиксированных upstream commits. Два изолированных TUN-контейнера приняли профили, завершили аутентифицированное рукопожатие и обменялись зашифрованными пакетами |
| Повтор установки | Настоящий Python-инициализатор из install.sh проверен в отдельном Docker-томе: новые ключи не заменяют существующие, чужой binding и другой протокол отклоняются |
| Shell | Синтаксис install/preflight/entrypoint и вспомогательных shell-скриптов проверен |
| Подготовка среды | `scripts/dev-setup.sh` выполнен; повторная установка зависимостей и сборка успешны. install_script/start_skill сохранены в черновик среды, не опубликованы |

## Не проверено и что нужно для приёмки

* Полная цепочка покупки/настройки на внешнем **чистом Ubuntu 24.04 VPS**: Docker, K3s, systemd, firewall, reboot, потеря SSH и повтор. В этой сессии внешний VPS и его доступ не предоставлялись. Проверки контейнеров не заменяют этот прогон.
* Настоящий оплаченный заказ ЮKassa, фискальный чек, отмена/возврат и production HTTPS billing. Аккаунт магазина, ключи и налоговая конфигурация не предоставлялись; деньги не списывались.
* Windows runtime, Android APK/device lifecycle, iOS IPA/симулятор/устройство, file picker/clipboard/QR import на всех ОС, магазинные покупки. CI добавлен, но в GitHub здесь не запускался.
* Внешний полный VPN-трафик, скорость, DNS/IPv6 leak tests, firewall провайдера и поведение REALITY в конкретной сети.

## Повторить

```sh
bash scripts/check.sh
bash scripts/protocol-check.sh       # нужны Docker и /dev/net/tun
bash scripts/native-check.sh         # Linux + Docker + rustup
python3 scripts/state-smoke.py       # использует образ myserv-native-check
python3 scripts/ui-smoke.py          # Vite на 1420, Python Playwright и Chromium
```

В managed cloud используйте штатный proxy/CA и разрешение executor на локальные сокеты. Во время установки `npm ci` остановите Vite, затем запустите его снова. Тестовый native-adapter существует только в `scripts/ui-smoke.py`, не в production-приложении.

Скриншоты: [desktop](screenshots/desktop.png), [mobile](screenshots/mobile.png).
