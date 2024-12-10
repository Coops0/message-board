const form = document.querySelector('#post-form');
const input = document.querySelector('#message');
const board = document.querySelector('.messages');

let encryptionKey = null;
let initialLoad = true;
let ws;
/**
 * @type {Map<string, Object>}
 */
let authorCache = new Map();

async function getEncryptionKey() {
    if (!encryptionKey) {
        const userIdBytes = new TextEncoder().encode(userId);
        const keyBytes = userIdBytes.slice(0, 16);
        encryptionKey = await crypto.subtle.importKey('raw', keyBytes, 'AES-CBC', false, ['encrypt']);
    }

    return encryptionKey;
}

/**
 * Creates a post element and appends it to the board.
 * @param {Object} message - The message object.
 * @param {string} message.content - The message content.
 * @param {string} [message.created_at] - The message creation date.
 * @param {boolean} [message.flagged] - Whether the message is flagged.
 * @param {boolean} [message.published] - Whether the message is published.
 * @param {string} [message.author] - The message author's ID.
 * @param {string} [message.id] - The message ID.
 * @param {boolean} self - Whether the message was sent by the user (and therefor contains only content)
 * @returns {HTMLElement} The created post element.
 */
function buildPost(message, self) {
    let html = '<div class="p-4 rounded-lg bg-zinc-800 border border-zinc-700/50">';

    // sanitize, remove any html
    const parser = new DOMParser();
    const doc = parser.parseFromString(message.content, 'text/html');

    html += '<p>' + doc.documentElement.innerText + '</p>';
    if (!self) {
        html += '<div class="mt-2 flex gap-2 message-controls">';

        const authorInfo = authorCache.get(message.author);

        if (!authorInfo) {
            html += '<button class="px-2 py-1 text-sm rounded bg-blue-600 hover:bg-blue-700 action-info">Fetch Info</button>';
            html += '<span class="text-gray-400 text-xs">' + message.author + '</span>';
        } else {
            html += '<div class="flex gap-2">';
            html += '<span>' + authorInfo.ip + '</span>';
            html += '<span>' + authorInfo.user_agent + '</span>';
            html += '</div>';
        }

        html += '<button class="px-2 py-1 text-sm rounded bg-green-600 hover:bg-green-700 action-publish" ' + (message.published ? 'disabled' : '') + '>' + (message.published ? 'Unpublish' : 'Publish') + '</button>';
        html += '<button class="px-2 py-1 text-sm rounded bg-red-600 hover:bg-red-700 action-ban">' + (authorInfo?.banned ? 'Unban' : 'Ban') + '</button>';
        html += '<button class="px-2 py-1 text-sm rounded bg-yellow-600 hover:bg-yellow-700 action-flag" ' + (message.flagged ? 'disabled' : '') + '>' + (message.flagged ? 'Unflag' : 'Flag') + '</button>';
    }

    html += '</div>';

    const post = document.createElement('div');
    post.innerHTML = html;

    if (!self) {
        post.dataset.id = message.id;
        post.dataset.type = 'message';
    }

    return post;
}

function updateMessages() {
    board.innerHTML = '';

    // todo action buttons are not hitting click
    // board.removeEventListener('click', handleActionClick, { capture: true });
    for (const message of messages) {
        // document.querySelectorAll(`[data-id="${message.id}"]`).forEach(e => e.removeEventListener('click', handleActionClick, { capture: true }));

        board.appendChild(buildPost(message, false));

        // document.querySelectorAll(`[data-id="${message.id}"]`).forEach(e => e.addEventListener('click', handleActionClick, { capture: true }));
    }

    // board.addEventListener('click', e => handleActionClick(e), { capture: true });
}

updateMessages();

form.addEventListener('submit', async e => {
    e.preventDefault();

    const text = input.value.trim();
    if (!text) {
        return;
    }

    buildPost({ content: text, id: window.crypto.randomUUID() }, true);
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

function websocket() {
    ws = new WebSocket((location.protocol === 'https:' ? 'wss' : 'ws') + '://' + location.host + '/-');
    ws.binaryType = 'arraybuffer';

    ws.onmessage = function ({ data }) {
        messages.unshift(JSON.parse(data));
        updateMessages();
    };

    ws.onclose = () => {
        setTimeout(() => websocket(), 1000);
    };
}

websocket();

async function handleActionClick(e) {
    const target = e.target;

    const messageElement = target.closest('[data-type="message"]');
    if (!messageElement) {
        return;
    }

    const { id } = messageElement.dataset;

    const messageId = messages.findIndex(m => m.id === id);
    let message = messages[messageId];

    let author = authorCache.get(message.author);

    if (target.matches('.action-info')) {
        if (!author) {
            author = await getUser(message.author);
            authorCache.set(message.author, author);
        }

        console.log('Author info:', author);
    }

    if (target.matches('.action-publish')) {
        const invertedPublish = !message.published;
        try {
            message = await updateMessage(message.id, { published: invertedPublish });
        } catch (e) {
            console.warn('Failed to update message:', e);
        }
    }

    if (target.matches('.action-ban')) {
        const newAuthor = await updateUser(message.author, { banned: !(author?.banned ?? false) });
        authorCache.set(message.author, newAuthor);
    }

    if (target.matches('.action-flag')) {
        const invertedFlag = !message.flagged;
        try {
            message = await updateMessage(message.id, { flagged: invertedFlag });
        } catch (e) {
            console.warn('Failed to update message:', e);
        }
    }

    console.log(message);
    messages[messageId] = message;

    updateMessages();
}

board.addEventListener('click', e => handleActionClick(e), { capture: true });

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