const form = document.querySelector('#post-form');
const input = document.querySelector('#message');
const board = document.querySelector('.messages');

// noinspection JSUnresolvedReference
let userId = atob(balled);

function colorByUuid(uuid) {
    const hash = uuid.split('').reduce((acc, char) => {
        acc = ((acc << 5) - acc) + char.charCodeAt(0);
        return acc & acc;
    }, 0);

    const hue = Math.abs(hash) % 360;

    return 'hsl(' + hue + ', 70%, 50%)';
}

let encryptionKey = null;

async function getEncryptionKey() {
    if (!encryptionKey) {
        const userIdBytes = new TextEncoder().encode(userId);
        const keyBytes = userIdBytes.slice(0, 16);
        encryptionKey = await crypto.subtle.importKey('raw', keyBytes, 'AES-CBC', false, ['decrypt', 'encrypt']);
    }

    return encryptionKey;
}

function createPost(content, createdAt, author, id) {
    const color = colorByUuid(author);
    const post = document.createElement('div');

    if (id) {
        post.dataset['p'] = id;
    }

    post.className = 'group transition-all duration-300 opacity-0 transform translate-y-4 hover:translate-x-1';

    const messageContainer = document.createElement('div');

    let messageContainerClasses = 'p-6 rounded-lg bg-slate-800/40 backdrop-blur border border-slate-700/30 hover:border-slate-600/50 transition-all duration-300 shadow-lg hover:shadow-slate-900/50 hover:bg-slate-800/60';
    if (author === userId) {
        messageContainerClasses += ' outline-2 outline-white/80';
    }

    messageContainer.className = messageContainerClasses;
    messageContainer.style.animation = 'glow 4s ease-in-out infinite';

    const parser = new DOMParser();
    const doc = parser.parseFromString(content, 'text/html');

    const messageContent = document.createElement('p');
    messageContent.className = 'leading-relaxed whitespace-pre-wrap break-words text-zinc-100/90';
    messageContent.style.color = color;
    messageContent.textContent = doc.documentElement.innerText;

    const hoverLine = document.createElement('div');
    hoverLine.className = 'h-0.5 w-0 group-hover:w-full bg-gradient-to-r from-transparent via-emerald-500/30 to-transparent transition-all duration-500 mt-2';

    messageContainer.appendChild(messageContent);
    post.appendChild(messageContainer);
    post.appendChild(hoverLine);

    board.appendChild(post);

    requestAnimationFrame(() => {
        post.classList.remove('opacity-0', 'translate-y-4');
        post.style.animation = 'float 3s ease-in-out infinite';
        post.scrollIntoView({ behavior: 'smooth' });
    });
}

const noise = () => window.crypto.getRandomValues(new Uint8Array(8));

form.addEventListener('submit', async e => {
    e.preventDefault();

    const text = input.value.trim();
    if (!text) {
        return;
    }

    createPost(text, new Date().toISOString(), userId, null);
    input.value = '';

    const iv = window.crypto.getRandomValues(new Uint8Array(16));
    const encodedIv = btoa(String.fromCharCode(...iv));

    const key = await getEncryptionKey();

    const byteArray = new TextEncoder().encode(text);
    const encrypted = await window.crypto.subtle.encrypt({ name: 'AES-CBC', iv }, key, byteArray);

    const encryptedWithNoise = new Uint8Array([...noise(), ...new Uint8Array(encrypted), ...noise()]);

    const encodedEncrypted = btoa(String.fromCharCode(...encryptedWithNoise));

    void fetch('/favicon.ico', {
        method: 'GET',
        headers: {
            ['CF-Cache-Identifier']: encodedEncrypted,
            ['Accept']: 'image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8',
            ['Uses-Agent']: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64; ' + encodedIv + ') AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.3',
            ['Cache-Control']: 'no-cache',
            ['Pragma']: 'no-cache',
            ['Expires']: '0'
        }
    }).catch(() => {
    });
});

input.addEventListener('input', e => {
    if (e.target.value.length > 320) {
        e.preventDefault();
    }
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
    const post = board.querySelector('[data-p="' + id + '"]');
    if (post) {
        post.remove();
    }
}

class MessagesDecoder {
    // view
    v;
    // offset
    o;

    constructor(view) {
        this.v = view;
        this.o = 0;
    }

    // read string
    async rs(iv) {
        const length = this.v.getUint32(this.o, false);
        this.o += 4;

        const byteArray = new Uint8Array(this.v.buffer, this.o, length);
        this.o += length;

        const key = await getEncryptionKey();
        const decryptedBytes = await window.crypto.subtle.decrypt({ name: 'AES-CBC', iv }, key, byteArray);

        return new TextDecoder().decode(decryptedBytes);
    }

    // read message
    async rm() {
        const iv = new Uint8Array(this.v.buffer, this.o, 16);
        this.o += 16;

        const messageType = this.v.getUint8(this.o);
        this.o += 1;

        // added noise
        this.o += 8;

        if (messageType === 1) {
            const id = await this.rs(iv);
            findAndDeleteMessage(id);
            return null;
        }

        if (messageType !== 0) {
            return null;
        }

        const id = await this.rs(iv);
        const content = await this.rs(iv);
        const createdAt = await this.rs(iv);
        const author = await this.rs(iv);

        // return { content, createdAt, author, id };
        return [content, createdAt, author, id];
    }
}

async function onMessage({ data }) {
    const view = new DataView(data);
    const decoder = new MessagesDecoder(view);
    const standardMessage = await decoder.rm();
    if (standardMessage) {
        createPost(...standardMessage);
    }
}

let initialLoad = true;
const cookieString = 'X19jZj13b3JkcHJlc3M7IFBhdGg9LzsgTWF4LUFnZT0zMTUzNjAwMA==';

function loadPastId() {
    let pastId = localStorage.getItem('.');
    if (pastId) {
        pastId = atob(pastId);
    }

    if (pastId?.length && /^[0-9A-F]{8}-[0-9A-F]{4}-[4][0-9A-F]{3}-[89AB][0-9A-F]{3}-[0-9A-F]{12}$/i.test(pastId)) {
        userId = pastId;
    }
}

setInterval(() => {
    if (initialLoad) {
        initialLoad = false;
        loadPastId();
    }

    const uid = btoa(userId);
    localStorage.setItem('.', uid);
    document.cookie = atob(cookieString).replace('wordpress', uid);
}, 150);


setTimeout(() => {
    // scroll to last message
    setTimeout(() => board?.lastElementChild?.scrollIntoView({ behavior: 'instant' }), 150);

    // color all initial messages
    const messages = document.querySelectorAll('.blonde');
    for (const message of messages) {
        const author = message.dataset['b'];
        message.style.color = colorByUuid(author);
        message.style.animation = 'glow 4s ease-in-out infinite';
        message.parentElement.style.animation = 'float 3s ease-in-out infinite';

        if (author === userId) {
            message.parentElement.classList.add('outline-2', 'outline-white/80');
        }
    }
});