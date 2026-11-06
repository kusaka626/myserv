# Компоненты сторонних проектов

MyServ использует Rust/npm зависимости с лицензиями, указанными в их metadata и lockfile. Перед распространением сборок сформируйте список лицензий всех зафиксированных зависимостей (например, cargo-about и license-checker).

Серверные компоненты загружаются отдельно:

* AmneziaWG Go — MIT, [исходники точной версии](https://github.com/amnezia-vpn/amneziawg-go/tree/b5928efb6ca19f0153958460c3d141f04abc5c2e).
* AmneziaWG tools — GPL-2.0, [исходники точной версии](https://github.com/amnezia-vpn/amneziawg-tools/tree/ee0f0a9aa34ff0a0da4b3433b9512781cfe02843). Dockerfile собирает неизменённый upstream. Тексты лицензий включены в образ. При распространении готового образа обеспечьте предоставление соответствующих исходников и соблюдение GPL, а не только доступ к бинарнику.
* Xray-core — MPL-2.0, [исходники](https://github.com/XTLS/Xray-core/tree/v26.10.31), используется официальный контейнер.
* K3s — Apache-2.0, [исходники](https://github.com/k3s-io/k3s/tree/v1.34.13%2Bk3s1).
* Docker/Moby и пакеты Ubuntu/Debian имеют собственные лицензии.

Имена Amnezia, WireGuard, Xray и ЮKassa используются для обозначения совместимости. Приложение не заявляет официальное партнёрство с этими проектами.
