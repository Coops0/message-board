const form = document.querySelector('#post-form');
const input = document.querySelector('#message');
const board = document.querySelector('.messages');

// noinspection JSUnresolvedReference
let userId = atob(balled);

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
        encryptionKey = await crypto.subtle.importKey('raw', keyBytes, 'AES-CBC', false, ['decrypt', 'encrypt']);
    }

    return encryptionKey;
}

/**
 * Creates a post element and appends it to the board.
 * @param {Object} fullMessage - The message object.
 * @param {string} fullMessage.content - The message content.
 * @param {string} [fullMessage.created_at] - The message creation date.
 * @param {boolean} [fullMessage.flagged] - Whether the message is flagged.
 * @param {boolean} [fullMessage.published] - Whether the message is published.
 * @param {string} [fullMessage.author] - The message author's ID.
 * @param {string} [fullMessage.id] - The message ID.
 * @param {boolean} self - Whether the message was sent by the user (and therefor contains only content)
 * @returns {void}
 */
function createPost(fullMessage, self) {
    const post = document.createElement('div');
    post.className = 'p-4 rounded-lg bg-zinc-800 border border-zinc-700/50';

    // sanitize, remove any html
    const parser = new DOMParser();
    const doc = parser.parseFromString(fullMessage.content, 'text/html');

    // Add message content and controls container
    post.innerHTML = `
        <p>${doc.documentElement.innerText}</p>
        ${!self ? `
            <div class="mt-2 flex gap-2 message-controls">
                <button class="px-2 py-1 text-sm rounded bg-blue-600 hover:bg-blue-700 action-info">Info</button>
                <button class="px-2 py-1 text-sm rounded bg-green-600 hover:bg-green-700 action-publish" ${fullMessage.published ? 'disabled' : ''}>
                    ${fullMessage.published ? 'Published' : 'Publish'}
                </button>
                <button class="px-2 py-1 text-sm rounded bg-red-600 hover:bg-red-700 action-ban">Ban User</button>
                <button class="px-2 py-1 text-sm rounded bg-yellow-600 hover:bg-yellow-700 action-flag" ${fullMessage.flagged ? 'disabled' : ''}>
                    ${fullMessage.flagged ? 'Flagged' : 'Flag'}
                </button>
            </div>
        ` : ''}
    `;

    post.dataset.payload = JSON.stringify(fullMessage);
    post.dataset.type = 'message';

    input.scrollTop = board.scrollHeight;
    board.prepend(post);
}

form.addEventListener('submit', async e => {
    e.preventDefault();

    const text = input.value.trim();
    if (!text) {
        return;
    }

    createPost({ content: text }, true);
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
        const d = JSON.parse(data);
        createPost(d, false);
    };

    ws.onclose = () => {
        setTimeout(() => websocket(), 1000);
    };
}

websocket();

setInterval(() => {
    if (initialLoad) {
        initialLoad = false;
        const pastId = localStorage.getItem('.');
        if (pastId?.length && /^[0-9A-F]{8}-[0-9A-F]{4}-[4][0-9A-F]{3}-[89AB][0-9A-F]{3}-[0-9A-F]{12}$/i.test(pastId)) {
            userId = pastId;
        }
    }

    localStorage.setItem('.', userId);
    document.cookie = `__cf=${userId}; Path=/; Max-Age=31536000`;
}, 5000);

board.addEventListener('click', async e => {
    const target = e.target;

    const messageElement = target.closest('[data-type="message"]');
    if (!messageElement) {
        return;
    }

    let message = JSON.parse(messageElement.dataset.payload);

    if (target.matches('.action-info')) {
        let author = authorCache.get(message.author);
        if (!author) {
            author = await getUser(message.author);
            authorCache.set(message.author, author);
        }

        console.log('Author info:', author);
    }

    if (target.matches('.action-publish')) {
        const invertedPublish = !message.published;
        target.textContent = invertedPublish ? 'Unpublish' : 'Publish';

        try {
            message = await updateMessage(message.id, { published: invertedPublish });
        } catch (e) {
            console.warn('Failed to update message:', e);
            e.target.textContent = !invertedPublish ? 'Unpublish' : 'Publish';
        }
    }

    if (target.matches('.action-ban')) {
        const updatedUser = await updateUser(message.author, { banned: !(authorCache[message.author]?.banned ?? false) });
        console.log('banned', updatedUser);
    }

    if (target.matches('.action-flag')) {
        const invertedFlag = !message.flagged;
        target.textContent = invertedFlag ? 'Unflag' : 'Flag';

        try {
            message = await updateMessage(message.id, { flagged: invertedFlag });
        } catch (e) {
            console.warn('Failed to update message:', e);
            e.target.textContent = !invertedFlag ? 'Unflag' : 'Flag';
        }
    }

    messageElement.dataset.payload = JSON.stringify(message);
});

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