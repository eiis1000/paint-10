import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { afterEach, test } from 'node:test';

// Exercise the actual inline wasm-bindgen adapter without building Rust or
// introducing a second JavaScript implementation for the tests.
const sourcePath = process.env.PAINT10_BROWSER_SOURCE
    ?? new URL('../src/web.rs', import.meta.url);
const source = readFileSync(sourcePath, 'utf8');
const inline = source.match(/inline_js = r#"([\s\S]*?)"#/);
assert.ok(inline, 'The browser adapter must expose its inline JavaScript module.');
const moduleUrl = `data:text/javascript;base64,${Buffer.from(inline[1]).toString('base64')}`;
const adapter = await import(moduleUrl);

const originalNavigator = Object.getOwnPropertyDescriptor(globalThis, 'navigator');
const originalWindow = Object.getOwnPropertyDescriptor(globalThis, 'window');
const originalKeyboardEvent = Object.getOwnPropertyDescriptor(globalThis, 'KeyboardEvent');

afterEach(() => {
    for (const [name, descriptor] of [
        ['navigator', originalNavigator],
        ['window', originalWindow],
        ['KeyboardEvent', originalKeyboardEvent],
    ]) {
        if (descriptor) {
            Object.defineProperty(globalThis, name, descriptor);
        } else {
            delete globalThis[name];
        }
    }
    adapter.takePastedImages();
});

function clipboard(items) {
    Object.defineProperty(globalThis, 'navigator', {
        configurable: true,
        value: { clipboard: { read: async () => items } },
    });
}

test('API text paste uses the same line endings as keyboard paste', async () => {
    clipboard([{
        types: ['text/plain'],
        getType: async () => new Blob(['First\r\n猫\r\nLast']),
    }]);
    assert.deepEqual(await adapter.readClipboard(), { text: 'First\n猫\nLast' });
});

test('denied API clipboard reads remain explicit errors', async () => {
    Object.defineProperty(globalThis, 'navigator', {
        configurable: true,
        value: {
            clipboard: {
                read: async () => {
                    throw new DOMException('Clipboard access denied', 'NotAllowedError');
                },
            },
        },
    });
    await assert.rejects(adapter.readClipboard(), { name: 'NotAllowedError' });
});

test('mixed-format clipboard contents prefer text only when editing text', async () => {
    const requested = [];
    clipboard([{
        types: ['image/png', 'text/plain'],
        getType: async type => {
            requested.push(type);
            return type === 'text/plain'
                ? new Blob(['Caption\r\n猫'])
                : new Blob([Uint8Array.of(1, 2, 3)]);
        },
    }]);
    assert.deepEqual(await adapter.readClipboard(true), { text: 'Caption\n猫' });
    assert.deepEqual(requested, ['text/plain']);
    requested.length = 0;
    assert.deepEqual(await adapter.readClipboard(false), { bytes: Uint8Array.of(1, 2, 3) });
    assert.deepEqual(requested, ['image/png']);
});

class Target {
    listeners = [];

    addEventListener(type, callback, options = {}) {
        this.listeners.push({ type, callback, capture: options.capture === true });
    }

    async dispatch(type, event, capture = false) {
        for (const listener of this.listeners) {
            if (listener.type === type && listener.capture === capture) {
                await listener.callback(event);
            }
        }
    }

    dispatchSync(type, event, capture = false) {
        for (const listener of this.listeners) {
            if (listener.type === type && listener.capture === capture) {
                listener.callback(event);
            }
        }
    }
}

function event(properties = {}) {
    return {
        ...properties,
        prevented: false,
        stopped: false,
        preventDefault() {
            this.prevented = true;
        },
        stopPropagation() {
            this.stopped = true;
        },
    };
}

test('image paste reaches Paint before the backend stops bubbling; text stays with the backend', async () => {
    const windowTarget = new Target();
    globalThis.window = windowTarget;
    let notifications = 0;
    adapter.installBrowserEvents(() => notifications++, new Target());

    let backendCalls = 0;
    const dispatchPaste = async paste => {
        await windowTarget.dispatch('paste', paste, true);
        if (!paste.stopped) {
            // Pinned eframe stops propagation at its document text listener,
            // even when an image clipboard contains no text.
            backendCalls++;
            paste.stopPropagation();
        }
        if (!paste.stopped) {
            await windowTarget.dispatch('paste', paste);
        }
    };

    const image = event({
        clipboardData: {
            items: [{
                type: 'image/png',
                getAsFile: () => ({
                    size: 3,
                    arrayBuffer: async () => Uint8Array.of(1, 2, 3).buffer,
                }),
            }],
        },
    });
    await dispatchPaste(image);
    assert.equal(image.prevented, true);
    assert.equal(backendCalls, 0);
    assert.equal(notifications, 1);
    assert.deepEqual(adapter.takePastedImages(), [Uint8Array.of(1, 2, 3)]);
    assert.deepEqual(adapter.takePastedImages(), []);

    const text = event({ clipboardData: { items: [{ type: 'text/plain' }] } });
    await dispatchPaste(text);
    assert.equal(text.prevented, false);
    assert.equal(backendCalls, 1);
    assert.equal(notifications, 1);

    const mixedText = event({
        target: { tagName: 'INPUT' },
        clipboardData: {
            items: image.clipboardData.items,
            getData: type => type === 'text/plain' ? 'Caption text' : '',
        },
    });
    await dispatchPaste(mixedText);
    assert.equal(mixedText.prevented, false);
    assert.equal(backendCalls, 2);
    assert.equal(notifications, 1);
    assert.deepEqual(adapter.takePastedImages(), []);
});

test('ruler shortcut suppresses reload without taking over F5 or hard reload', async () => {
    globalThis.window = new Target();
    const canvas = new Target();
    adapter.installBrowserEvents(() => {}, canvas);

    for (const [properties, expected] of [
        [{ key: 'r', ctrlKey: true }, true],
        [{ key: 'R', metaKey: true }, true],
        [{ key: 'r', ctrlKey: true, shiftKey: true }, false],
        [{ key: 'F5' }, false],
        [{ key: 's', ctrlKey: true }, false],
    ]) {
        const key = event(properties);
        await canvas.dispatch('keydown', key, true);
        assert.equal(key.prevented, expected);
    }
});

test('F10 stays with Paint keytips while context-menu and IME shortcuts keep their defaults', async () => {
    const windowTarget = new Target();
    globalThis.window = windowTarget;
    const canvas = new Target();
    let pageFocused = true;
    canvas.ownerDocument = { hasFocus: () => pageFocused };
    adapter.installBrowserEvents(() => {}, canvas);

    for (const [properties, expected] of [
        [{ key: 'F10', target: { tagName: 'CANVAS' } }, true],
        [{ key: 'F10', target: { tagName: 'INPUT' } }, true],
        [{ key: 'F10', shiftKey: true }, false],
        [{ key: 'F10', ctrlKey: true }, false],
        [{ key: 'F10', altKey: true }, false],
        [{ key: 'F10', metaKey: true }, false],
        [{ key: 'F10', isComposing: true }, false],
        [{ key: 'F10', keyCode: 229 }, false],
        [{ key: 'F10', pageFocused: false }, false],
        [{ key: 'F5' }, false],
    ]) {
        pageFocused = properties.pageFocused ?? true;
        const key = event(properties);
        await windowTarget.dispatch('keydown', key, true);
        assert.equal(key.prevented, expected, JSON.stringify(properties));
        assert.equal(key.stopped, false, 'eframe must still receive the key event');
    }
});

test('F12 opens Save As without also opening browser DevTools', async () => {
    const windowTarget = new Target();
    globalThis.window = windowTarget;
    const canvas = new Target();
    let pageFocused = true;
    canvas.ownerDocument = { hasFocus: () => pageFocused };
    adapter.installBrowserEvents(() => {}, canvas);

    for (const [properties, expected] of [
        [{ target: { tagName: 'CANVAS' } }, true],
        [{ target: { tagName: 'INPUT' } }, true],
        [{ shiftKey: true }, false],
        [{ ctrlKey: true }, false],
        [{ altKey: true }, false],
        [{ metaKey: true }, false],
        [{ isComposing: true }, false],
        [{ keyCode: 229 }, false],
        [{ pageFocused: false }, false],
    ]) {
        pageFocused = properties.pageFocused ?? true;
        const key = event({ key: 'F12', ...properties });
        await windowTarget.dispatch('keydown', key, true);
        assert.equal(key.prevented, expected, JSON.stringify(properties));
        assert.equal(key.stopped, false, 'Paint must still receive the Save As shortcut');
    }
});

function keyboardBrowser() {
    const windowTarget = new Target();
    globalThis.window = windowTarget;
    globalThis.KeyboardEvent = class {
        constructor(type, properties) {
            return event({ type, ...properties });
        }
    };

    const document = { hasFocus: () => focused, activeElement: null };
    let focused = true;
    const received = [];
    const makeTarget = (tagName, type) => {
        const target = new Target();
        Object.assign(target, { tagName, type, ownerDocument: document, isConnected: true });
        target.dispatchEvent = key => {
            key.target = target;
            windowTarget.dispatchSync(key.type, key, true);
            if (!key.stopped) {
                // The backend accepts both real keys and the bridge's F10,
                // but pure modifier events do not queue an egui key.
                received.push({ key: key.key, type: key.type, target, prevented: key.prevented });
            }
            return !key.prevented;
        };
        return target;
    };
    const canvas = makeTarget('CANVAS');
    const input = makeTarget('INPUT', 'text');
    document.activeElement = canvas;
    adapter.installBrowserEvents(() => {}, canvas);

    return {
        canvas,
        input,
        document,
        received,
        focusPage: value => { focused = value; },
        cancel: type => windowTarget.dispatchSync(type, event(), true),
        send(type, properties = {}) {
            const key = event({
                key: 'Alt',
                altKey: type === 'keydown',
                type,
                ...properties,
            });
            (properties.target ?? document.activeElement).dispatchEvent(key);
            return key;
        },
    };
}

test('Paint history keys prevent browser-native edits while reaching the Rust backend', () => {
    for (const targetName of ['canvas', 'input']) {
        for (const command of [{ ctrlKey: true }, { metaKey: true }]) {
            for (const shortcut of [
                { key: 'z' },
                { key: 'y' },
                { key: 'Z', shiftKey: true },
                { key: 'z', repeat: true },
            ]) {
                const browser = keyboardBrowser();
                const target = browser[targetName];
                browser.document.activeElement = target;
                const key = browser.send('keydown', {
                    altKey: false,
                    ...command,
                    ...shortcut,
                });
                assert.equal(key.prevented, true, `${targetName}: ${JSON.stringify(shortcut)}`);
                assert.equal(key.stopped, false, 'Rust must still receive the history command');
                assert.equal(browser.received.length, 1);
                assert.equal(browser.received[0].key, shortcut.key);
                assert.equal(browser.received[0].target, target);
                assert.equal(browser.received[0].prevented, true);
            }
        }
    }
});

test('history capture preserves IME, other shortcuts, and unrelated or unfocused targets', () => {
    for (const properties of [
        { ctrlKey: false },
        { shiftKey: true, key: 'Y' },
        { metaKey: true },
        { altKey: true },
        { isComposing: true },
        { keyCode: 229 },
        { getModifierState: name => name === 'AltGraph' },
        { key: 'a' },
        { key: 'c' },
        { key: 'v' },
        { key: 'x' },
        { key: 'F5' },
    ]) {
        const browser = keyboardBrowser();
        const key = browser.send('keydown', {
            key: 'z',
            ctrlKey: true,
            altKey: false,
            ...properties,
        });
        assert.equal(key.prevented, false, JSON.stringify(properties));
        assert.equal(key.stopped, false);
    }

    for (const [name, configure] of [
        ['unfocused page', browser => browser.focusPage(false)],
        ['inactive target', browser => { browser.document.activeElement = browser.canvas; }],
        ['removed target', browser => { browser.input.isConnected = false; }],
        ['foreign document', browser => { browser.input.ownerDocument = {}; }],
        ['file chooser', browser => { browser.input.type = 'file'; }],
        ['unrelated textarea', browser => { browser.input.tagName = 'TEXTAREA'; }],
        ['unrelated canvas', browser => { browser.input.tagName = 'CANVAS'; }],
    ]) {
        const browser = keyboardBrowser();
        browser.document.activeElement = browser.input;
        configure(browser);
        const key = browser.send('keydown', {
            target: browser.input,
            key: 'z',
            ctrlKey: true,
            altKey: false,
        });
        assert.equal(key.prevented, false, name);
        assert.equal(key.stopped, false, name);
    }
});

test('a standalone Alt tap queues exactly one keytip toggle before subsequent letters', async () => {
    for (const targetName of ['canvas', 'input']) {
        for (const pause of [false, true]) {
            const browser = keyboardBrowser();
            const target = browser[targetName];
            browser.document.activeElement = target;
            browser.send('keydown');
            browser.send('keydown', { repeat: true });
            if (pause) {
                // An intervening frame must not change which DOM event carries
                // the toggle. Rust separately tests an already-seen Alt-down.
                await Promise.resolve();
            }
            browser.send('keyup');
            browser.send('keyup');
            browser.send('keydown', { key: 'h', altKey: false });
            browser.send('keyup', { key: 'h' });
            const keys = browser.received.filter(key => key.key !== 'Alt');
            assert.deepEqual(keys.map(key => [key.type, key.key]), [
                ['keydown', 'F10'],
                ['keyup', 'F10'],
                ['keydown', 'h'],
                ['keyup', 'h'],
            ]);
            assert.ok(keys.every(key => key.target === target));
            assert.equal(keys[0].prevented, true, 'The generated F10 keeps browser menu focus away');
        }
    }
});

test('Alt chords, IME, pointer actions and lost focus do not become standalone taps', () => {
    const cases = [
        ['Alt+H', browser => browser.send('keydown', { key: 'h' })],
        ['Ctrl+Alt', browser => browser.send('keydown', { ctrlKey: true })],
        ['Shift+Alt', browser => browser.send('keydown', { shiftKey: true })],
        ['Meta+Alt', browser => browser.send('keydown', { metaKey: true })],
        ['AltGraph', browser => browser.send('keydown', {
            getModifierState: name => name === 'AltGraph',
        })],
        ['IME key', browser => browser.send('keydown', { isComposing: true })],
        ['IME compatibility key', browser => browser.send('keydown', { keyCode: 229 })],
        ['composition begins', browser => browser.cancel('compositionstart')],
        ['pointer action', browser => browser.cancel('pointerdown')],
        ['focus leaves and returns', browser => browser.cancel('blur')],
        ['page loses focus', browser => browser.focusPage(false)],
        ['target changes', browser => { browser.document.activeElement = browser.input; }],
        ['target removed', browser => { browser.canvas.isConnected = false; }],
    ];
    for (const [name, interrupt] of cases) {
        const browser = keyboardBrowser();
        browser.send('keydown');
        interrupt(browser);
        browser.send('keyup');
        assert.ok(browser.received.every(key => key.key !== 'F10'), name);
    }

    for (const properties of [
        { shiftKey: true },
        { ctrlKey: true },
        { metaKey: true },
        { isComposing: true },
        { keyCode: 229 },
        { repeat: true },
        { getModifierState: name => name === 'AltGraph' },
    ]) {
        const browser = keyboardBrowser();
        browser.send('keydown', properties);
        browser.send('keyup');
        assert.ok(browser.received.every(key => key.key !== 'F10'), JSON.stringify(properties));
    }

    const unpaired = keyboardBrowser();
    unpaired.send('keyup');
    assert.ok(unpaired.received.every(key => key.key !== 'F10'));

    const unfocused = keyboardBrowser();
    unfocused.focusPage(false);
    unfocused.send('keydown');
    unfocused.focusPage(true);
    unfocused.send('keyup');
    assert.ok(unfocused.received.every(key => key.key !== 'F10'));

    for (const properties of [
        { shiftKey: true },
        { ctrlKey: true },
        { metaKey: true },
        { altKey: true },
        { isComposing: true },
        { keyCode: 229 },
        { getModifierState: name => name === 'AltGraph' },
    ]) {
        const browser = keyboardBrowser();
        browser.send('keydown');
        browser.send('keyup', properties);
        assert.ok(browser.received.every(key => key.key !== 'F10'), JSON.stringify(properties));
    }

    const fileInput = keyboardBrowser();
    fileInput.input.type = 'file';
    fileInput.document.activeElement = fileInput.input;
    fileInput.send('keydown');
    fileInput.send('keyup');
    assert.ok(fileInput.received.every(key => key.key !== 'F10'));
});
