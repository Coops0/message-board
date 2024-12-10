const form = document.querySelector('#post-form');
const input = document.querySelector('#message');
const board = document.querySelector('.messages');

let encryptionKey = null;
let ws;

const colorCache = new Map();
const authorCache = new Map();

async function getEncryptionKey() {
    if (!encryptionKey) {
        const userIdBytes = new TextEncoder().encode(userId);
        const keyBytes = userIdBytes.slice(0, 16);
        encryptionKey = await crypto.subtle.importKey('raw', keyBytes, 'AES-CBC', false, ['encrypt']);
    }

    return encryptionKey;
}

function createPostElement(message, self) {
    let color = colorCache.get(message.author);
    if (!color) {
        color = `hsl(${Math.floor(Math.random() * 360)}, 100%, 50%)`;
        colorCache.set(message.author, color);
    }

    const post = document.createElement('div');
    post.className = 'p-4 rounded-lg bg-zinc-800 border border-zinc-700 ' + !self && !message.published ? 'opacity-20' : 'opacity-80';

    const parser = new DOMParser();
    const doc = parser.parseFromString(message.content, 'text/html');
    const content = document.createElement('p');
    content.style.color = color;
    content.textContent = doc.documentElement.innerText;
    post.appendChild(content);

    if (!self) {
        const controls = document.createElement('div');
        controls.className = 'mt-3 flex items-center gap-2';

        const authorInfo = authorCache.get(message.author);

        if (!authorInfo) {
            const infoButton = createButton('Info', 'info', message.id);
            controls.appendChild(infoButton);

            const authorSpan = document.createElement('span');
            authorSpan.className = 'text-zinc-400 text-sm';
            authorSpan.textContent = message.author;
            controls.appendChild(authorSpan);
        } else {
            const info = document.createElement('div');
            info.className = 'flex gap-2 text-zinc-400 text-sm truncate';
            info.innerHTML = `<span>${authorInfo.ip}</span><span>${authorInfo.user_agent}</span>`;
            controls.appendChild(info);
        }

        controls.appendChild(createButton(message.published ? 'Unpublish' : 'Publish', 'publish', message.id));
        controls.appendChild(createButton(authorInfo?.banned ? 'Unban' : 'Ban', 'ban', message.id));
        controls.appendChild(createButton(message.flagged ? 'Unflag' : 'Flag', 'flag', message.id));

        post.appendChild(controls);
        post.dataset.id = message.id;
        post.dataset.type = 'message';
    }

    return post;
}

const styles = {
    info: 'bg-blue-600 hover:bg-blue-700',
    publish: 'bg-green-600 hover:bg-green-700',
    ban: 'bg-red-600 hover:bg-red-700',
    flag: 'bg-yellow-600 hover:bg-yellow-700'
};

function createButton(text, action, messageId) {
    const button = document.createElement('button');
    button.className = `px-3 py-1.5 rounded text-sm font-medium transition-colors ${styles[action]}`;
    button.textContent = text;
    button.dataset.action = action;
    button.dataset.messageId = messageId;

    return button;
}

async function handleAction(action, messageId) {
    const messageIndex = messages.findIndex(m => m.id === messageId);
    if (messageIndex === -1) {
        return;
    }

    let message = messages[messageIndex];
    let author = authorCache.get(message.author);

    switch (action) {
        case 'info':
            if (!author) {
                author = await getUser(message.author);
                authorCache.set(message.author, author);
            }
            break;

        case 'publish':
            message = await updateMessage(messageId, { published: !message.published });
            break;

        case 'ban':
            author = await updateUser(message.author, { banned: !(author?.banned ?? false) });
            authorCache.set(message.author, author);
            break;

        case 'flag':
            message = await updateMessage(messageId, { flagged: !message.flagged });
            break;
    }

    messages[messageIndex] = { ...message };
    updateMessages();
}

function updateMessages() {
    board.innerHTML = '';
    for (const message of messages) {
        board.appendChild(createPostElement(message));
    }
}

board.addEventListener('click', async (e) => {
    const button = e.target.closest('button[data-action]');
    if (!button) {
        return;
    }

    const { action, messageId } = button.dataset;
    await handleAction(action, messageId);
});

form.addEventListener('submit', async e => {
    e.preventDefault();

    const text = input.value.trim();
    if (!text) {
        return;
    }

    const messageId = crypto.randomUUID();
    board.insertBefore(
        createPostElement({ content: text, id: messageId }, true),
        board.firstChild
    );
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

function connectWebSocket() {
    const protocol = location.protocol === 'https:' ? 'wss' : 'ws';
    ws = new WebSocket(`${protocol}://${location.host}/-`);

    ws.onmessage = ({ data }) => {
        messages.unshift(JSON.parse(data));
        updateMessages();
    };

    ws.onclose = () => setTimeout(connectWebSocket, 1000);
}

// Initialize
updateMessages();
connectWebSocket();

/**
 * Fetches a user with the given ID.
 * @param id
 * @returns {Promise<Object | null>} The user object.
 */
const getUser = async (id) => {
    const response = await fetch(`/admin/user/${id}`);
    return await response.json();
};

/**
 * Updates a user with the given payload
 * @param id
 * @param {Object} payload
 * @param {boolean} [payload.banned]
 * @returns {Promise<Object>} The updated user.
 */
const updateUser = async (id, payload) => {
    const response = await fetch(`/admin/user/${id}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload)
    });

    return await response.json();
};

/**
 * Updates a message with the given payload
 *
 * @param {string} id
 * @param {Object} payload
 * @param {string} [payload.content]
 * @param {boolean} [payload.flagged] - Whether the message is flagged.
 * @param {boolean} [payload.published] - Whether the message is published.
 * @returns {Promise<Object>} The updated message object
 */
const updateMessage = async (id, payload) => {
    const response = await fetch(`/admin/message/${id}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload)
    });

    return await response.json();
};