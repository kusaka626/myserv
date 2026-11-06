"""Browser interaction check. Native calls are mocked ONLY in this test, never in the app."""
import os
from playwright.sync_api import sync_playwright
free_preview = os.environ.get("MYSERV_TEST_FREE_PREVIEW") == "1"
with sync_playwright() as p:
    browser=p.chromium.launch(executable_path='/usr/bin/chromium',headless=True,args=['--no-sandbox'])
    page=browser.new_page(viewport={'width':1280,'height':900})
    errors=[]
    page.on('pageerror',lambda e:errors.append(str(e)))
    page.add_init_script('window.myservTestFree = ' + str(free_preview).lower() + ';')
    page.add_init_script('''
      window.isTauri = true;
      window.__TAURI_INTERNALS__ = {
        transformCallback: () => 1,
        invoke: async (cmd, args) => {
          if (cmd === 'settings') return {free_preview:window.myservTestFree,billing_ready:!window.myservTestFree,mobile:false,version:'test'};
          if (window.myservTestFree && ['create_payment','check_payment','open_payment'].includes(cmd)) throw new Error('Payment called in free mode');
          if (window.myservTestFree && cmd === 'deploy' && args.license !== '') throw new Error('Free mode should not forge a license');
          if (cmd === 'plugin:event|listen') return 1;
          if (cmd === 'connect') {
            if (Object.keys(args).sort().join(',') !== 'ip,password,port,username') throw new Error('Unexpected fields');
            if (args.username !== 'ubuntu' || args.port !== 2222 || args.password !== 'test-only') throw new Error('Wrong credentials');
            return {session:'test',summary:'Сервер подходит для установки'};
          }
          if (cmd === 'create_payment') return {status:'pending',confirmation_url:'https://yookassa.ru/test',license:null};
          if (cmd === 'open_payment') return null;
          if (cmd === 'check_payment') return {status:'succeeded',confirmation_url:null,license:'test.signature'};
          if (cmd === 'deploy') return {filename:'myserv-awg.conf',content:'test-only',import_uri:null,qr_svg:'<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200"><rect width="200" height="200" fill="white"/></svg>'};
          if (cmd === 'export_file') return true;
          if (cmd === 'disconnect') return null;
          throw new Error('Unexpected command '+cmd);
        }
      };
    ''')
    page.goto('http://127.0.0.1:1420')
    page.get_by_role('button',name='Сервер куплен, продолжить').click()
    page.get_by_placeholder('203.0.113.10').fill('203.0.113.10')
    assert page.locator('main input').count() == 4
    page.get_by_label('Логин',exact=True).fill('ubuntu')
    page.get_by_label('Порт',exact=True).fill('2222')
    page.get_by_placeholder('Пароль из панели хостинга').fill('test-only')
    page.get_by_role('button',name='Подключить сервер',exact=True).click()
    page.get_by_role('heading',name='Как будем подключаться?').wait_for()
    page.get_by_role('button',name='Продолжить',exact=True).click()
    if free_preview:
        page.get_by_role('heading',name='Всё готово к установке').wait_for()
        assert page.get_by_label('Email для чека').count() == 0
        assert 'Оплатить настройку' not in page.locator('nav').inner_text()
    else:
        page.get_by_label('Email для чека').fill('fixture@example.com')
        page.get_by_role('button',name='Оплатить 199 ₽').click()
        page.get_by_role('button',name='Проверить оплату').click()
    page.get_by_role('button',name='Настроить мой сервер').click()
    assert 'Подтвердите разрешение' in page.get_by_role('alert').inner_text()
    page.get_by_label('Это мой чистый VPS').check()
    page.get_by_role('button',name='Настроить мой сервер').click()
    page.get_by_role('heading',name='Ваш сервер настроен').wait_for()
    page.get_by_role('button',name='Сохранить myserv-awg.conf').click()
    page.get_by_text('Файл сохранён.').wait_for()
    page.get_by_role('button',name='Завершить и очистить SSH-доступ').click()
    page.get_by_role('heading',name='Ваш сервер. Ваши правила.').wait_for()
    assert not errors, errors
    browser.close()
    print('UI wizard with mocked native boundary: PASS; no real payment or VPS provisioning performed')
