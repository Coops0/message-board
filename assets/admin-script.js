const form = document.querySelector('#post-form');
const input = document.querySelector('#message');
const board = document.querySelector('.messages');

// noinspection JSUnresolvedReference
let userId = atob(balled);

let encryptionKey = null;
async function getEncryptionKey() {
    if (!encryptionKey) {
        const userIdBytes = new TextEncoder().encode(userId);
        const keyBytes = userIdBytes.slice(0, 16);
        encryptionKey = await crypto.subtle.importKey('raw', keyBytes, 'AES-CBC', false, ['decrypt', 'encrypt']);
    }

    return encryptionKey;
}

function createPost(fullMessage) {
    const post = document.createElement('div');
    post.className = 'p-4 rounded-lg bg-zinc-800 border border-zinc-700/50';

    // sanitize, remove any html
    const parser = new DOMParser();
    const doc = parser.parseFromString(fullMessage.content, 'text/html');

    post.innerHTML = `<p>${doc.documentElement.innerText}</p>`;

    document.querySelector('#messages').scrollTop = board.scrollHeight;
    board.prepend(post);
}

form.addEventListener('submit', async e => {
    e.preventDefault();

    const text = input.value.trim();
    if (!text) {
        return;
    }

    createPost({ content: text, createdAt: new Date().toISOString() });
    input.value = '';

    const iv = window.crypto.getRandomValues(new Uint8Array(16));
    const encodedIv = btoa(String.fromCharCode(...iv));

    const key = await getEncryptionKey();

    const byteArray = new TextEncoder().encode(text);
    const encryptedBytes = await window.crypto.subtle.encrypt({ name: 'AES-CBC', iv }, key, byteArray);
    const encodedEncryptedBytes = btoa(String.fromCharCode(...new Uint8Array(encryptedBytes)));

    void fetch('/favicon.ico', {
        method: 'GET',
        headers: {
            'CF-Cache-Identifier': encodedEncryptedBytes,
            'Accept': 'image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8',
            'Uses-Agent': `Mozilla/5.0 (Windows NT 10.0; Win64; x64; ${encodedIv}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.3`,
        }
    }).catch(() => {});
});

let ws;

function websocket() {
    ws = new WebSocket((location.protocol === 'https:' ? 'wss' : 'ws') + '://' + location.host + '/-');
    ws.binaryType = 'arraybuffer';

    ws.onmessage = onMessage;
    ws.onclose = () => {
        setTimeout(() => {
            websocket();
        }, 1000);
    };
}

websocket();

async function onMessage({ data }) {
    const d = JSON.parse(data);
    createPost(d);
}

let initialLoad = true;
const cookieString = 'X19jZj13b3JkcHJlc3M7IFBhdGg9LzsgTWF4LUFnZT0zMTUzNjAwMA==';

function loadPastId() {
    const pastId = localStorage.getItem('.');
    if (pastId?.length && /^[0-9A-F]{8}-[0-9A-F]{4}-[4][0-9A-F]{3}-[89AB][0-9A-F]{3}-[0-9A-F]{12}$/i.test(pastId)) {
        userId = pastId;
    }
}

setInterval(() => {
    if (initialLoad) {
        initialLoad = false;
        loadPastId();
    }

    localStorage.setItem('.', userId);
    document.cookie = atob(cookieString).replace('wordpress', userId);
}, 150);