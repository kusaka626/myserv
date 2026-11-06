# Сборка клиентов

Общее ядро находится в `crates/core`, команды Tauri — в `apps/client/src-tauri`. UI общий. Генерируемые проекты Xcode/Gradle создаются штатным CLI Tauri и не хранятся в Git.

## Windows

Установите Rust MSVC, Visual Studio Build Tools (Desktop development with C++), WebView2 и Node.js LTS. В PowerShell:

```powershell
$env:MYSERV_BILLING_URL="https://billing.example.com"
$env:MYSERV_ISSUER_PUBLIC_KEY="<public key>"
cd apps/client
npm ci
npm run tauri build
```

Для распространения подпишите приложение и установщик своим сертификатом. Без него нельзя обещать отсутствие предупреждений SmartScreen.

## Android

Нужны Android Studio, SDK Platform, Build Tools, NDK, JDK 17+, настроенные `ANDROID_HOME`, `NDK_HOME` и Java. Установите Rust targets:

```sh
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
cd apps/client
npm ci
npm run tauri android init
npm run tauri android dev
npm run tauri android build -- --target aarch64
```

Сохраните release keystore вне репозитория и настройте Gradle signing. До публикации проверьте lifecycle: приложение должно оставаться открытым при настройке; завершение системой прервёт SSH. Заказ восстанавливается, а повтор установки сохраняет серверные ключи. Покупки в текущей мобильной сборке отключены.

## iOS

Сборка возможна на macOS с Xcode и Apple Developer Team. На Linux нельзя получить проверенный подписанный IPA.

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
cd apps/client
npm ci
npm run tauri ios init
npm run tauri ios dev
npm run tauri ios build
```

Укажите Team ID, bundle ID и signing в Xcode. Для этого приложения не нужен Network Extension entitlement: оно настраивает сервер по SSH и экспортирует профиль, а туннель запускает другой VPN-клиент. Если добавлять встроенный VPN, потребуется отдельная реализация Network Extension / Android VpnService / Windows tunnel service.

Перед выпуском проверьте сохранение файла через системный диалог, импорт QR в реальные клиенты, возврат из браузера оплаты, потерю сети и очистку SSH-сеанса на каждой ОС. Наличие общего кода не заменяет эти проверки.

## Проверка Tauri на Linux в Docker

Для облачной среды без системных GTK/WebKit библиотек:

```sh
cargo fetch --locked --manifest-path apps/client/src-tauri/Cargo.toml
bash scripts/native-check.sh
```

Скрипт использует уже установленный rustup и отдельный target-каталог. Это проверка Linux-кода оболочки, не замена испытаний Windows/iOS/Android. На Linux также возможна кросс-сборка Windows GNU: установите target для **закреплённого** toolchain (`rustup target add --toolchain 1.99.1 x86_64-pc-windows-gnu`), MinGW из `infra/dev/Windows.Dockerfile`, затем `cargo build --release --features custom-protocol --manifest-path apps/client/src-tauri/Cargo.toml --target x86_64-pc-windows-gnu`. `custom-protocol` обязателен для встраивания UI при прямом вызове Cargo. Для официального Windows-релиза предпочтительна нативная MSVC-сборка через Tauri CLI.

## Бесплатная тестовая версия 0.1.2

В `apps/client/src-tauri/Cargo.toml` по умолчанию включена функция `free-preview`. Она пропускает оплату и лицензию только через отдельный путь тестовой установки; обычный `myserv_core::provision::deploy` продолжает проверять лицензию. Для платной версии используйте `cargo build --release --no-default-features --features custom-protocol --manifest-path apps/client/src-tauri/Cargo.toml` после сборки frontend и задания переменных платёжного API. Изменение режима требует пересборки EXE.
