import "./style.css";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  createIcons,
  ArrowRight,
  ArrowLeft,
  Check,
  CheckCheck,
  Server,
  ShieldCheck,
  KeyRound,
  BookOpen,
  Terminal,
  CreditCard,
  Download,
  Copy,
  ExternalLink,
  ChevronRight,
  CircleHelp,
  LockKeyhole,
  Layers,
  Zap,
  Globe,
  X,
  LoaderCircle,
} from "lucide";
import {
  escapeHtml as esc,
  validIp,
  canDeploy,
  steps,
  type Step,
} from "./flow";
const icons = {
  ArrowRight,
  ArrowLeft,
  Check,
  CheckCheck,
  Server,
  ShieldCheck,
  KeyRound,
  BookOpen,
  Terminal,
  CreditCard,
  Download,
  Copy,
  ExternalLink,
  ChevronRight,
  CircleHelp,
  LockKeyhole,
  Layers,
  Zap,
  Globe,
  X,
  LoaderCircle,
};
const icon = (name: string, cls = "") =>
  `<i data-lucide="${name}" class="${cls}" aria-hidden="true"></i>`;
const app = document.querySelector<HTMLDivElement>("#app")!;
interface Profile {
  filename: string;
  content: string;
  import_uri: string | null;
  qr_svg: string;
}
interface Payment {
  status: string;
  confirmation_url: string | null;
  license: string | null;
}
let step: Step = "guide",
  busy = false,
  error = "",
  notice = "",
  session = "",
  summary = "",
  license = "";
let ip = "",
  port = 22,
  username = "root",
  email = "",
  protocol = "amnezia_wg",
  domain = "";
let settings = {
  free_preview: false,
  billing_ready: false,
  mobile: false,
  version: "0.1.2",
};
let profile: Profile | null = null,
  paymentUrl = "",
  activeStage = "preflight";
let lastFailed = false;
const labels = [
  "Купить VPS",
  "Подключить сервер",
  "Выбрать протокол",
  "Оплатить настройку",
  "Запустить VPN",
  "Получить доступ",
];
const titles: Record<Step, string> = {
  guide: "Ваш сервер. Ваши правила.",
  server: "Познакомимся с вашим сервером",
  protocol: "Как будем подключаться?",
  payment: "Один сервер. Одна оплата.",
  install: "Собираем ваш VPN-сервер",
  ready: "Ваш сервер настроен",
};
const subtitles: Record<Step, string> = {
  guide:
    "Поднимите личный VPN, даже если никогда не открывали терминал. Мы объясним каждый шаг и возьмём настройку на себя.",
  server:
    "Укажите данные из панели хостинга. Соединение пойдёт напрямую с устройства на ваш VPS.",
  protocol:
    "Оба варианта работают на вашем VPS. Настройки и ключи мы подготовим автоматически.",
  payment:
    "199 ₽ за автоматическую настройку вашего VPS. Без подписки на VPN и платы за трафик со стороны MyServ.",
  install:
    "Оставьте приложение открытым. Первая установка может занять 10–20 минут.",
  ready:
    "Контейнер запущен. Импортируйте конфигурацию в совместимый клиент и проверьте подключение.",
};
function render() {
  const visibleSteps = steps.filter(
    (s) => !settings.free_preview || s !== "payment",
  );
  const index = visibleSteps.indexOf(step);
  app.innerHTML = `<aside class="sidebar"><a class="brand" href="#" id="home"><span class="brandmark">m<span>›</span></span>myserv<span class="beta">preview</span></a><div class="sidebar-label">СВОЙ VPN · ШАГ ЗА ШАГОМ</div><nav aria-label="Этапы настройки">${visibleSteps.map((s, i) => `<div class="nav-step ${i === index ? "active" : ""} ${i < index ? "complete" : ""}"><span class="step-number">${i < index ? icon("check") : String(i + 1).padStart(2, "0")}</span><span>${labels[steps.indexOf(s)]}</span>${i === index ? icon("chevron-right") : ""}</div>`).join("")}</nav><div class="ownership"><span class="icon-tile">${icon("shield-check")}</span><strong>Сервер принадлежит вам</strong><p>Вы выбираете хостинг, храните ключи и управляете доступом.</p></div><div class="sidebar-footer"><span class="status-dot"></span> Windows · iOS · Android <small>Единое ядро на Rust · ${settings.version}</small></div></aside><div class="workspace"><header><div class="breadcrumbs">Личный сервер ${icon("chevron-right")} <span>${labels[steps.indexOf(step)]}</span></div><button class="help" id="help">${icon("circle-help")} Как это работает</button></header><main><div class="eyebrow"><span></span> ${step === "guide" ? "НЕЗАВИСИМОСТЬ НАЧИНАЕТСЯ ЗДЕСЬ" : `ШАГ ${index + 1} ИЗ ${visibleSteps.length}`}</div><h1>${titles[step]}</h1><p class="intro">${subtitles[step]}</p>${!isTauri() ? '<div class="browser-note">Открыт веб-интерфейс для знакомства. SSH и настройка сервера доступны в установленном приложении.</div>' : ""}${error ? `<div class="alert" role="alert">${icon("circle-help")}<span>${esc(error)}</span></div>` : ""}${notice ? `<div class="notice" role="status">${esc(notice)}</div>` : ""}${settings.free_preview ? '<div class="notice">Тестовая сборка: настройка бесплатна, оплата и лицензия не требуются.</div>' : ""}${body()}<footer class="main-footer">${icon("lock-keyhole")} SSH-данные остаются на устройстве и не передаются в платёжный сервис.</footer></main></div>`;
  createIcons({ icons });
  bind();
}
function body(): string {
  if (step === "guide")
    return `<section class="hero-card"><div><span class="pill">ВМЕСТО ГОТОВОГО VPN — СВОЙ</span><h2>От пустого VPS<br>до первого подключения.</h2><p>Вы арендуете сервер. MyServ устанавливает<br class="desktop-only"> всё необходимое и выдаёт доступ.</p><div class="hero-tags"><span>${icon("check")} Без терминала</span><span>${icon("check")} Ключи под вашим контролем</span></div></div><div class="server-art" aria-hidden="true"><div class="orbit orbit-one"></div><div class="orbit orbit-two"></div><div class="rack"><div><span></span><span></span><b>01</b></div><div><span></span><span></span><b>02</b></div><div><span></span><span></span><b>03</b></div></div><div class="secure-chip">${icon("shield-check")} ваш сервер</div><span class="art-star">✳</span></div></section><div class="section-heading"><h2>Сначала — арендуем сервер</h2><span>~5 минут</span></div><div class="guide-grid"><article><span class="guide-num">01</span><h3>Выберите хостинг</h3><p>Любой провайдер с VPS, публичным IPv4 и разрешённым VPN-трафиком. Выберите удобную страну размещения.</p></article><article><span class="guide-num">02</span><h3>Закажите чистый VPS</h3><p>Ubuntu 24.04 LTS · 2 vCPU · 2 ГБ RAM · 20 ГБ SSD. Без предустановленной панели управления.</p></article><article><span class="guide-num">03</span><h3>Сохраните SSH-доступ</h3><p>В панели получите IPv4, логин и пароль. Разрешите SSH и порт выбранного протокола в firewall хостинга.</p></article></div><details class="details"><summary>${icon("book-open")} Что именно я покупаю и сколько это стоит?</summary><p>Аренду VPS вы оплачиваете выбранному хостингу отдельно, обычно ежемесячно. ${settings.free_preview ? "В этой тестовой сборке настройка бесплатна." : "MyServ берёт 199 ₽ один раз за настройку одного сервера."} Приложение установит Docker, Kubernetes K3s и один выбранный протокол. Готовую конфигурацию можно импортировать в AmneziaWG 3.1 или совместимый Xray-клиент. Эта версия не включает встроенный VPN-туннель.</p><p>Для REALITY откройте 443/TCP. Для AmneziaWG — 51820/UDP. Порт SSH обычно 22/TCP. API Kubernetes будет доступен только внутри VPS.</p></details><div class="action-row"><div><strong>${settings.free_preview ? "Бесплатно" : "199 ₽"} <span>/ настройка</span></strong><small>Аренда VPS оплачивается отдельно</small></div><button class="primary" id="next">Сервер куплен, продолжить ${icon("arrow-right")}</button></div>`;
  if (step === "server")
    return `<section class="panel"><div class="panel-title">${icon("server")} Данные сервера <span class="tag">Ubuntu 24.04</span></div><p class="hint">Введите данные из панели хостинга. Мы подключимся и проверим сервер автоматически.</p><div class="form-grid"><label>IP сервера<input id="ip" value="${esc(ip)}" placeholder="203.0.113.10" inputmode="decimal" autocomplete="off" ${busy ? "disabled" : ""}></label><label>Порт<input id="port" type="number" min="1" max="65535" value="${port}" ${busy ? "disabled" : ""}></label></div><label>Логин<input id="username" value="${esc(username)}" placeholder="root" autocomplete="off" autocapitalize="none" spellcheck="false" ${busy ? "disabled" : ""}></label><label>Пароль<input id="password" type="password" autocomplete="off" placeholder="Пароль из панели хостинга" ${busy ? "disabled" : ""}></label><p class="hint">Обычно логин — root. Если хостинг выдал другой логин, укажите его. Пароль не сохраняется на диске.</p></section><div class="action-row"><button class="text-button" id="back" ${busy ? "disabled" : ""}>${icon("arrow-left")} Назад</button><button class="primary" id="connect" ${busy ? "disabled" : ""}>${icon(busy ? "loader-circle" : "shield-check", busy ? "spin" : "")} ${busy ? "Подключаемся и проверяем…" : "Подключить сервер"}</button></div>`;
  if (step === "protocol")
    return `<div class="connected">${icon("check-check")} ${esc(ip)} <span>${esc(summary)}</span></div><div class="protocol-grid"><button class="protocol-card ${protocol === "amnezia_wg" ? "selected" : ""}" data-protocol="amnezia_wg"><span class="selection-dot"></span><span class="protocol-icon">${icon("zap")}</span><span class="tag">Быстрое подключение</span><h2>AmneziaWG <em>3.1</em></h2><p>Для повседневного использования, звонков и видео. Обфускация трафика на основе WireGuard.</p><div class="protocol-bottom">UDP · порт 51820 <strong>.conf + QR</strong></div></button><button class="protocol-card ${protocol === "vless_reality" ? "selected" : ""}" data-protocol="vless_reality"><span class="selection-dot"></span><span class="protocol-icon">${icon("globe")}</span><span class="tag">TLS-транспорт</span><h2>VLESS <em>+ REALITY</em></h2><p>Xray с защищённым транспортом REALITY. Работает поверх TCP, нужен подходящий TLS-домен.</p><div class="protocol-bottom">TCP · порт 443 <strong>ссылка + QR</strong></div></button></div>${protocol === "vless_reality" ? `<section class="panel"><label>Домен назначения REALITY<input id="domain" value="${esc(domain)}" placeholder="Домен с поддержкой TLS 1.3 и HTTP/2" spellcheck="false"></label><p class="hint">Домен должен поддерживать TLS 1.3 и HTTP/2 и быть доступен с VPS. Приложение проверит его при установке; доступность примера не гарантирована.</p></section>` : ""}<div class="info-strip">${icon("layers")} Docker-контейнер под управлением Kubernetes K3s. Один сервер, один выбранный протокол.</div><div class="action-row"><button class="text-button" id="back">${icon("arrow-left")} Назад</button><button class="primary" id="next">Продолжить ${icon("arrow-right")}</button></div>`;
  if (step === "payment")
    return `<section class="checkout"><div><span class="pill">РАЗОВАЯ НАСТРОЙКА</span><h2>Личный VPN-сервер</h2><p>${esc(ip)} · ${protocol === "amnezia_wg" ? "AmneziaWG 3.1" : "VLESS + REALITY"}</p><ul class="features"><li>${icon("check")} Подготовка Ubuntu, Docker и Kubernetes</li><li>${icon("check")} Установка и запуск выбранного протокола</li><li>${icon("check")} Персональная конфигурация и QR-код</li><li>${icon("check")} Повтор установки на том же сервере</li></ul></div><div class="price-card"><span>К оплате</span><strong>199 <small>₽</small></strong><p>Один раз · без подписки</p><span class="payment-brand">Оплата через <b>ЮKassa</b></span></div></section>${!settings.billing_ready ? '<div class="browser-note">В этой сборке платёжный сервис ещё не подключён. Разработчику нужно настроить аккаунт ЮKassa и адрес API. Демонстрационная оплата не выдаёт доступ.</div>' : ""}${settings.mobile ? '<div class="browser-note">Покупки в мобильной сборке пока не подключены. Здесь можно использовать ранее полученную лицензию для этого VPS.</div>' : ""}<section class="panel"><label>Email для чека<input id="email" type="email" value="${esc(email)}" placeholder="you@example.com" autocomplete="email"></label><div class="payment-actions"><button class="primary" id="pay" ${busy || !settings.billing_ready || settings.mobile ? "disabled" : ""}>${icon("credit-card")} Оплатить 199 ₽</button>${paymentUrl ? '<button class="secondary" id="check-payment">Проверить оплату</button>' : ""}</div>${paymentUrl ? '<p class="hint">Оплатите в открывшемся окне ЮKassa и вернитесь сюда. Не создавайте повторный заказ.</p>' : ""}<details><summary>У меня уже есть лицензия</summary><label>Подписанный ключ настройки<textarea id="license" rows="3" spellcheck="false" placeholder="Вставьте полученную лицензию">${esc(license)}</textarea></label><button class="secondary" id="use-license">Использовать лицензию</button></details></section><div class="action-row"><button class="text-button" id="back">${icon("arrow-left")} Назад</button><span class="subtle">VPS остаётся в вашем аккаунте хостинга</span></div>`;
  if (step === "install")
    return `<section class="panel install-panel"><span class="big-icon">${icon(busy ? "loader-circle" : "terminal", busy ? "spin" : "")}</span><h2>${busy ? "Настраиваем сервер…" : lastFailed ? "Установка остановлена" : "Всё готово к установке"}</h2><p>${busy ? "Не закрывайте приложение и не выключайте устройство." : "Будут установлены Docker и K3s, создан контейнер и настроена сеть выбранного протокола."}</p><ol class="install-steps">${[
      ["preflight", "Проверка сервера"],
      ["packages", "Системные пакеты и Docker"],
      ["kubernetes", "Kubernetes K3s"],
      ["image", "Контейнер протокола"],
      ["deploy", "Настройка и запуск"],
      ["ready", "Получение конфигурации"],
    ]
      .map(
        ([s, t]) =>
          `<li class="${s === activeStage && busy ? "current" : ""}">${icon(s === activeStage && busy ? "loader-circle" : "layers", s === activeStage && busy ? "spin" : "")} ${t}</li>`,
      )
      .join(
        "",
      )}</ol>${!busy ? `<label class="checkbox"><input type="checkbox" id="consent"> Это мой чистый VPS. Разрешаю установить ПО и изменить сетевые настройки.</label><button class="primary" id="deploy">${lastFailed ? "Повторить с сохранением ключей" : "Настроить мой сервер"} ${icon("arrow-right")}</button>` : ""}<p class="hint">${protocol === "amnezia_wg" ? "AmneziaWG использует привилегированный контейнер для сетевого интерфейса. IPv6 направляется в туннель и блокируется этой IPv4-конфигурацией." : "Xray запускается без root. API Kubernetes закрыт от внешнего доступа."}</p></section>`;
  return profile
    ? `<section class="success-banner">${icon("check-check")} <div><strong>Настройка завершена</strong><span>Теперь проверьте реальное подключение в VPN-клиенте.</span></div><span class="tag">${esc(ip)}</span></section><div class="result-grid"><section class="panel qr-panel"><img src="data:image/svg+xml,${encodeURIComponent(profile.qr_svg)}" alt="QR-код вашей конфигурации VPN"><h3>Отсканируйте в VPN-клиенте</h3><p>QR содержит секретные ключи. Не публикуйте его.</p></section><section class="panel"><h2>Ваш доступ к серверу</h2><p class="hint">${protocol === "amnezia_wg" ? "Импортируйте файл в клиент с поддержкой AmneziaWG 3.1. Обычный WireGuard не подходит." : "Добавьте VLESS-ссылку в совместимый клиент Xray с REALITY."}</p><button class="primary full" id="download">${icon("download")} Сохранить ${esc(profile.filename)}</button><button class="secondary full" id="copy">${icon("copy")} ${profile.import_uri ? "Скопировать VLESS-ссылку" : "Скопировать конфигурацию"}</button>${settings.free_preview ? "" : '<button class="text-button full" id="save-license">Сохранить лицензию для повторной настройки</button>'}<details><summary>Посмотреть конфигурацию</summary><textarea readonly rows="8" spellcheck="false">${esc(profile.content)}</textarea></details></section></div><section class="panel"><h3>Последний шаг — проверить подключение</h3><p class="hint">Включите VPN в выбранном клиенте и проверьте доступ к интернету и внешний IP. При отсутствии связи проверьте firewall хостинга и выбранный порт. Готовность контейнера сама по себе не подтверждает прохождение трафика.</p><button class="secondary" id="finish">Завершить и очистить SSH-доступ</button></section>`
    : "";
}
function val(id: string) {
  return (document.getElementById(id) as HTMLInputElement | null)?.value ?? "";
}
function on(id: string, fn: () => void) {
  document.getElementById(id)?.addEventListener("click", fn);
}
async function action(fn: () => Promise<void>) {
  if (busy) return;
  error = "";
  notice = "";
  busy = true;
  render();
  try {
    await fn();
  } catch (e) {
    error = e instanceof Error ? e.message : String(e);
  } finally {
    busy = false;
    render();
  }
}
function native() {
  if (!isTauri())
    throw new Error(
      "Для подключения по SSH запустите установленное приложение MyServ. В браузере доступна инструкция.",
    );
}
async function save(name: string, content: string) {
  if (isTauri()) {
    const saved = await invoke<boolean>("export_file", {
      filename: name,
      content,
    });
    notice = saved
      ? "Файл сохранён. Храните его в безопасном месте."
      : "Сохранение отменено";
    return;
  }
  const blob = new Blob([content], { type: "text/plain;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 5000);
}
function bind() {
  on("home", () => {
    if (!busy && !session) {
      step = "guide";
      render();
    }
  });
  on("help", () => {
    notice = `Купите чистый VPS → введите IP, логин, пароль и порт → выберите протокол → ${settings.free_preview ? "запустите настройку бесплатно" : "оплатите настройку"} → импортируйте конфигурацию. Аренда сервера оплачивается хостингу отдельно.`;
    render();
  });
  on("next", () => {
    if (step === "protocol") {
      domain = val("domain") || domain;
      if (protocol === "vless_reality" && !domain) {
        error = "Укажите домен назначения REALITY.";
        render();
        return;
      }
    }
    step =
      step === "protocol" && settings.free_preview
        ? "install"
        : steps[steps.indexOf(step) + 1];
    error = "";
    render();
  });
  on("back", () => {
    if (busy) return;
    error = "";
    notice = "";
    if (step === "protocol") {
      void action(async () => {
        await invoke("disconnect");
        session = "";
        step = "server";
      });
    } else {
      step = steps[Math.max(0, steps.indexOf(step) - 1)];
      render();
    }
  });
  document.querySelectorAll<HTMLButtonElement>("[data-protocol]").forEach(
    (b) =>
      (b.onclick = () => {
        domain = val("domain") || domain;
        protocol = b.dataset.protocol!;
        render();
      }),
  );
  on("connect", () => {
    ip = val("ip").trim();
    port = Number(val("port"));
    username = val("username").trim();
    let password = val("password");
    void action(async () => {
      try {
        native();
        if (!validIp(ip) || port < 1 || port > 65535 || !Number.isInteger(port))
          throw new Error("Укажите IPv4 и порт от 1 до 65535.");
        if (!username || !password)
          throw new Error("Введите логин и пароль сервера.");
        const c = await invoke<{ session: string; summary: string }>(
          "connect",
          {
            ip,
            port,
            username,
            password,
          },
        );
        session = c.session;
        summary = c.summary;
        step = "protocol";
      } finally {
        password = "";
      }
    });
  });
  on("pay", () => {
    email = val("email");
    void action(async () => {
      native();
      if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email))
        throw new Error("Укажите email для чека.");
      const p = await invoke<Payment>("create_payment", {
        sessionId: session,
        email,
      });
      await applyPayment(p);
      if (paymentUrl) await invoke("open_payment", { url: paymentUrl });
    });
  });
  on(
    "check-payment",
    () =>
      void action(async () => {
        await applyPayment(
          await invoke<Payment>("check_payment", { sessionId: session }),
        );
      }),
  );
  on("use-license", () => {
    license = val("license").trim();
    if (license) {
      step = "install";
      error = "";
      render();
    }
  });
  on("deploy", () => {
    const accepted = (document.getElementById("consent") as HTMLInputElement)
      .checked;
    void action(async () => {
      if (!canDeploy(session, license, accepted, settings.free_preview))
        throw new Error("Подтвердите разрешение на настройку своего VPS.");
      lastFailed = true;
      profile = await invoke<Profile>("deploy", {
        sessionId: session,
        options: { protocol, reality_domain: domain },
        license,
      });
      lastFailed = false;
      step = "ready";
    });
  });
  on(
    "download",
    () =>
      void action(async () => {
        if (profile) await save(profile.filename, profile.content);
      }),
  );
  on(
    "save-license",
    () =>
      void action(async () => {
        await save("myserv-license.txt", license);
      }),
  );
  on(
    "copy",
    () =>
      void action(async () => {
        if (profile) {
          await navigator.clipboard.writeText(
            profile.import_uri || profile.content,
          );
          notice =
            "Скопировано. Передавайте конфигурацию только на свои устройства.";
        }
      }),
  );
  on(
    "finish",
    () =>
      void action(async () => {
        await invoke("disconnect");
        session = "";
        profile = null;
        license = "";
        paymentUrl = "";
        step = "guide";
        notice =
          "SSH-доступ и конфигурация удалены из текущего сеанса. Сервер продолжает работать.";
      }),
  );
}
async function applyPayment(p: Payment) {
  if (p.license) {
    license = p.license;
    step = "install";
    paymentUrl = "";
  } else {
    paymentUrl = p.confirmation_url || paymentUrl;
    notice =
      p.status === "canceled"
        ? "Платёж отменён. Деньги не подтверждены."
        : "Оплата ещё не подтверждена. Вернитесь после оплаты и нажмите «Проверить оплату».";
  }
}
if (isTauri()) {
  void invoke<typeof settings>("settings").then((s) => {
    settings = s;
    render();
  });
  void listen<{ step: string; message: string }>("provision-progress", (e) => {
    activeStage = e.payload.step;
    notice = e.payload.message;
    render();
  });
}
window.addEventListener("beforeunload", (event) => {
  if (session) {
    event.preventDefault();
    event.returnValue = "";
  }
});
render();
