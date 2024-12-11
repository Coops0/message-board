const form = document.querySelector('#post-form');
const input = document.querySelector('#message');
const board = document.querySelector('.messages');

// noinspection JSUnresolvedReference
let userId = atob(balled);

const authorColors = new Map();

let encryptionKey = null;

async function getEncryptionKey() {
    if (!encryptionKey) {
        const userIdBytes = new TextEncoder().encode(userId);
        const keyBytes = userIdBytes.slice(0, 16);
        encryptionKey = await crypto.subtle.importKey('raw', keyBytes, 'AES-CBC', false, ['decrypt', 'encrypt']);
    }

    return encryptionKey;
}

function createPost({ content, createdAt, author, id }) {
    let color = authorColors.get(author);
    if (!color) {
        const hue = Math.floor(Math.random() * 360);
        color = `hsl(${hue}, 70%, 80%)`;
        authorColors.set(author, color);
    }

    const post = document.createElement('div');
    if (id) {
        post.dataset['p'] = id;
    }

    post.className = 'group transition-all duration-300 opacity-0 transform translate-y-4';

    const messageContainer = document.createElement('div');
    messageContainer.className = 'p-5 rounded-lg bg-slate-800/50 backdrop-blur border border-slate-700/30 hover:border-slate-600/50 transition-all duration-150 shadow-lg hover:shadow-slate-900/50';

    const parser = new DOMParser();
    const doc = parser.parseFromString(content, 'text/html');

    const messageContent = document.createElement('p');
    messageContent.className = 'leading-relaxed';
    messageContent.style.color = color;
    messageContent.textContent = doc.documentElement.innerText;

    const hoverLine = document.createElement('div');
    hoverLine.className = 'h-px w-0 group-hover:w-full bg-gradient-to-r from-transparent via-slate-500/50 to-transparent transition-all duration-500 mt-1';

    messageContainer.appendChild(messageContent);
    post.appendChild(messageContainer);
    post.appendChild(hoverLine);

    board.appendChild(post);

    requestAnimationFrame(() => {
        post.classList.remove('opacity-0', 'translate-y-4');
    });

    post.scrollIntoView({ behavior: 'smooth' });
}

form.addEventListener('submit', async e => {
    e.preventDefault();

    const text = input.value.trim();
    if (!text) {
        return;
    }

    createPost({ content: text, createdAt: new Date().toISOString(), author: userId });
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
            'Uses-Agent': `Mozilla/5.0 (Windows NT 10.0; Win64; x64; ${encodedIv}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.3`
        }
    }).catch(() => {
    });
});

let ws;

function websocket() {
    ws = new WebSocket((location.protocol === 'https:' ? 'wss' : 'ws') + '://' + location.host + '/-');
    ws.binaryType = 'arraybuffer';

    ws.onmessage = onMessage;
    ws.onclose = () => {
        setTimeout(() => websocket(), 1000);
    };
}

websocket();

function findAndDeleteMessage(id) {
    const post = board.querySelector(`[data-p="${id}"]`);
    if (post) {
        post.remove();
    }
}

class MessagesDecoder {
    view;
    offset;

    constructor(view) {
        this.view = view;
        this.offset = 0;
    }

    async readString(iv) {
        const length = this.view.getUint32(this.offset, false);
        this.offset += 4;

        const byteArray = new Uint8Array(this.view.buffer, this.offset, length);
        this.offset += length;

        const key = await getEncryptionKey();
        const decryptedBytes = await window.crypto.subtle.decrypt({ name: 'AES-CBC', iv }, key, byteArray);

        return new TextDecoder().decode(decryptedBytes);
    }

    async readMessage() {
        const iv = new Uint8Array(this.view.buffer, this.offset, 16);
        this.offset += 16;

        const messageType = this.view.getUint8(this.offset);
        this.offset += 1;

        if (messageType === 1) {
            const id = await this.readString(iv);
            findAndDeleteMessage(id);
            return null;
        }

        if (messageType !== 0) {
            console.warn('unknown message type', messageType);
            return null;
        }

        const id = await this.readString(iv);
        const content = await this.readString(iv);
        const createdAt = await this.readString(iv);
        const author = await this.readString(iv);

        return { content, createdAt, author, id };
    }
}


async function onMessage({ data }) {
    const view = new DataView(data);
    const decoder = new MessagesDecoder(view);
    const standardMessage = await decoder.readMessage();
    if (standardMessage) {
        createPost(standardMessage);
    }
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


setTimeout(() => {
    // scroll to last message
    try {
        board?.lastElementChild?.scrollIntoView({ behavior: 'instant' });
    } catch {

    }

    // color all initial messages
    const messages = document.querySelectorAll('.blonde');
    for (const message of messages) {
        const author = message.dataset['b'];
        let color = authorColors.get(author);
        if (!color) {
            const hue = Math.floor(Math.random() * 360);
            color = `hsl(${hue}, 70%, 80%)`;
            authorColors.set(author, color);
        }

        message.style.color = color;
    }
});