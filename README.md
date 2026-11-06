# MyServ

[Скачать приложение для Windows (ZIP)](https://github.com/kusaka626/myserv/raw/refs/heads/main/downloads/myserv-windows-x64-preview.zip) · [Инструкция запуска предварительной сборки](downloads/README.md)

Приложение для обучения и автоматической настройки **собственного VPN-сервера**. Пользователь покупает VPS, подтверждает его SSH-ключ, выбирает AmneziaWG 3.1 или Xray VLESS+REALITY, оплачивает настройку за **199 ₽** через ЮKassa и получает конфигурацию/QR. VPS оплачивается отдельно. Это preview-версия: см. [проверки и ограничения](docs/VALIDATION.md).

## Запуск разработки

Нужны Rust (версия закреплена в `rust-toolchain.toml`) и Node.js 22.12+.

```sh
cargo test --workspace --locked
cd apps/client
npm ci
npm test
npm run dev              # только веб-интерфейс; SSH доступен в нативной сборке
npm run tauri dev        # приложение с общим Rust-ядром
```

На Linux для Tauri нужны `build-essential pkg-config libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev`. На Windows нужны Rust MSVC, Visual Studio C++ Build Tools и WebView2. Платформенные инструкции: [BUILD.md](docs/BUILD.md).

Не подключайте рабочий сервер с существующими сервисами: текущий установщик рассчитан на **чистую Ubuntu 24.04** с root, systemd, публичным IPv4, TUN, 2 ГБ RAM и 8 ГБ свободного места. Рекомендуется 2 vCPU, 20 ГБ SSD. Откройте SSH и 51820/UDP для AWG либо 443/TCP для REALITY в firewall хостинга. K3s API не открывается наружу.

## Устройство репозитория

```text
crates/core/            общее Rust-ядро: SSH, профили, лицензии, установка
crates/cli/             CLI для диагностики и автоматизированного запуска
apps/client/           адаптивный интерфейс и нативная оболочка Tauri 2
services/billing/      Rust/Axum API, SQLite, ЮKassa, подпись лицензий
infra/                 установка Docker/K3s, контейнер AmneziaWG
scripts/               воспроизводимые проверки
```

SSH-пароли и приватные SSH-ключи не отправляются в billing и не записываются приложением на диск. Они хранятся в памяти Rust-сеанса до завершения/закрытия. Локально сохраняется заказ оплаты с токеном восстановления (app data, права 0600 на Unix); после оплаты сохраните лицензию. VPN-конфигурация хранится в root-only состоянии VPS и экспортируется только по действию пользователя.

```sh
cargo run -p myserv-cli -- fingerprint <VPS_IP> 22
```

Сравните SHA256 с ключом через независимую консоль хостинга. Первый полученный по сети ключ **не считается доверенным автоматически**.

Для `inspect request.json` и `deploy request.json --output profile.conf` CLI принимает JSON с полями `server`, `credentials`, `options`, `license`. См. модели в `crates/core/src/model.rs`; файл содержит секреты, храните его с ограниченными правами. CLI не перезаписывает существующий файл экспорта. И CLI, и UI используют один установщик и одну проверку лицензии.

## Оплата и выпуск

Без настроенного API кнопка оплаты отключена. Нет фиктивной успешной оплаты или обхода проверки лицензии в production. Инструкция для оператора: [BILLING.md](docs/BILLING.md).

Для сборки платёжного клиента задайте **во время сборки**:

```sh
MYSERV_BILLING_URL=https://billing.your-domain.example
MYSERV_ISSUER_PUBLIC_KEY=<base64url-public-key>
```

Секрет ЮKassa и приватный ключ подписи находятся только на billing-сервере. Общий код Windows/iOS/Android подготовлен; мобильные покупки требуют выбранной модели дистрибуции и соответствующей интеграции. Встроенного VPN-туннеля в MyServ нет: используйте выданный профиль в совместимом клиенте.

Подробнее: [исследование](docs/RESEARCH.md), [сборка](docs/BUILD.md), [эксплуатация и восстановление](docs/OPERATIONS.md), [результаты проверок](docs/VALIDATION.md).
