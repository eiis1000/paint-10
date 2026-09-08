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

afterEach(() => {
    for (const [name, descriptor] of [
        ['navigator', originalNavigator],
        ['window', originalWindow],
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
