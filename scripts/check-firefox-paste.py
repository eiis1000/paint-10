#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["selenium"]
# ///
"""Diagnose real Firefox image-paste events on a private X11 clipboard.

Run with geckodriver, xclip, Xvfb, and Firefox available. Reports go under
tmp/firefox-paste; the user's desktop and clipboard are never used.
"""
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import time
import zlib

from selenium import webdriver
from selenium.webdriver import ActionChains
from selenium.webdriver.common.keys import Keys
from selenium.webdriver.firefox.options import Options
from selenium.webdriver.support.ui import WebDriverWait


def png(width, height):
    def chunk(kind, data):
        return struct.pack('!I', len(data)) + kind + data + struct.pack('!I', zlib.crc32(kind + data))
    pixels = (b'\0' + bytes([235, 35, 55]) * width) * height
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('!2I5B', width, height, 8, 2, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(pixels)) + chunk(b'IEND', b''))


def main():
    out = Path('tmp/firefox-paste')
    out.mkdir(parents=True, exist_ok=True)
    read_fd, write_fd = os.pipe()
    display = subprocess.Popen(['Xvfb', '-displayfd', str(write_fd), '-screen', '0', '1280x900x24', '-nolisten', 'tcp'],
                               pass_fds=(write_fd,), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    os.close(write_fd)
    clipboard = None
    try:
        with os.fdopen(read_fd) as pipe:
            os.environ['DISPLAY'] = ':' + pipe.readline().strip()
        os.environ.pop('WAYLAND_DISPLAY', None)
        os.environ['MOZ_ENABLE_WAYLAND'] = '0'
        options = Options()
        options.binary_location = shutil.which('firefox')
        options.set_preference('webgl.force-enabled', True)
        options.set_preference('gfx.webrender.software', True)
        with webdriver.Firefox(options=options) as browser:
            browser.set_window_size(1200, 850)
            browser.get('https://eiis1000.github.io/paint-10/')
            WebDriverWait(browser, 90).until(lambda d: d.execute_script('return !!window.paint10 && document.querySelector("#loading").hidden'))
            browser.execute_script('''
                window.pasteEvents = [];
                window.addEventListener('paste', event => pasteEvents.push({
                    target: event.target.tagName, id: event.target.id,
                    types: [...event.clipboardData.types],
                    items: [...event.clipboardData.items].map(item => ({type:item.type,kind:item.kind,size:item.getAsFile()?.size}))
                }), true);
            ''')
            clipboard = subprocess.Popen(['xclip', '-quiet', '-selection', 'clipboard', '-target', 'image/png'],
                                         stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            clipboard.stdin.write(png(200, 100))
            clipboard.stdin.close()
            time.sleep(0.3)
            canvas = browser.find_element('id', 'paint')
            canvas.click()
            ActionChains(browser).key_down(Keys.CONTROL).send_keys('v').key_up(Keys.CONTROL).perform()
            time.sleep(1)
            report = {'version': browser.capabilities['browserVersion'], 'canvas': browser.execute_script('return {events:pasteEvents,active:document.activeElement.outerHTML}')}
            browser.save_screenshot(str(out / 'canvas-paste.png'))
            browser.execute_script('''
                window.pasteEvents = [];
                const input = document.createElement('textarea');
                input.id = 'paste-probe';
                input.style = 'position:fixed;top:250px;left:300px;z-index:999';
                document.body.append(input); input.focus();
            ''')
            ActionChains(browser).key_down(Keys.CONTROL).send_keys('v').key_up(Keys.CONTROL).perform()
            time.sleep(1)
            report['textarea'] = browser.execute_script('return pasteEvents')
            browser.execute_script('''
                window.pasteEvents = [];
                const input = document.createElement('div');
                input.contentEditable = 'true'; input.textContent = 'Paste here';
                input.style = 'position:fixed;top:400px;left:300px;z-index:999';
                document.body.append(input); input.focus();
            ''')
            ActionChains(browser).key_down(Keys.CONTROL).send_keys('v').key_up(Keys.CONTROL).perform()
            time.sleep(1)
            report['contenteditable'] = browser.execute_script('return pasteEvents')
            (out / 'events.json').write_text(json.dumps(report, indent=2))
            print(json.dumps(report, indent=2))
    finally:
        if clipboard:
            clipboard.terminate()
        display.terminate()
        display.wait(timeout=10)


if __name__ == '__main__':
    main()
