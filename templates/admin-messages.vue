<!DOCTYPE html>
<html lang="en" class="dark">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>walt whitman msgboard</title>
  <style>'{{ TAILWIND_STYLES }}'</style>
</head>
<body>
<div id="app">
  <div class="bg-zinc-900 text-zinc-100 min-h-screen">
    <div class="mx-auto max-w-4xl h-screen flex flex-col">
      <header class="bg-zinc-800/90 backdrop-blur-lg border-b border-zinc-700/50 px-6 py-4 sticky top-0 z-10">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-4">
            <h1 class="text-lg font-semibold">MSGBOARD</h1>
            <div class="h-4 w-px bg-zinc-700"></div>
            <div class="flex gap-4 text-sm">
              <span class="text-zinc-400">Messages: {{ messages.length }}</span>
              <span class="text-red-400">Flagged: {{ messages.filter(m => m.flagged).length }}</span>
              <span class="text-yellow-400">Unpublished: {{
                  messages.filter(m => !m.published && !m.self).length
                }}</span>
            </div>
          </div>
        </div>
      </header>

      <div class="flex-1 overflow-y-auto p-6">
        <div class="space-y-4">
          <div
              v-for="(message, index) in messages"
              :key="message.id"
              class="group relative"
              ref="messages-ref"
          >
            <div :class="[
              'p-4 rounded-lg border transition-all',
              'bg-zinc-800/80 hover:bg-zinc-800',
              message.flagged ? 'border-red-500/50' : (message.published ? 'border-green-500/50' : 'border-zinc-700/50'),
              (!message.self && !message.published) ? 'opacity-50' : 'opacity-100'
            ]">
              <div class="flex items-start gap-3">
                <div class="flex-1">
                  <div class="flex justify-between items-start">
                    <p :style="{ color: getMessageColor(message.author) }"
                       class="text-lg break-all whitespace-pre-wrap m-2">
                      {{ message.content }}
                    </p>
                    <div v-if="message.created_at" class="text-xs text-zinc-500">
                      {{ formatRelativeTime(message.created_at) }}
                    </div>
                  </div>

                  <div v-if="!message.self && authorInfo[message.author]" class="mt-3 space-y-2">
                    <div class="flex items-center gap-3 text-sm">
                      <div class="flex items-center gap-2 px-3 py-1.5 rounded-md bg-zinc-700/50">
                        <svg class="w-4 h-4 text-zinc-400" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                             stroke-width="2">
                          <path
                              d="M9 3H5a2 2 0 0 0-2 2v4m6-6h10a2 2 0 0 1 2 2v4M9 3v18m0 0h10a2 2 0 0 0 2-2V9M9 21H5a2 2 0 0 1-2-2V9m0 0h18"/>
                        </svg>
                        <span class="text-zinc-300">{{ authorInfo[message.author].ip }}</span>
                      </div>

                      <div v-if="authorInfo[message.author].user_agent"
                           class="flex items-center gap-2 px-3 py-1.5 rounded-md bg-zinc-700/50"
                           :title="authorInfo[message.author].user_agent">
                        <svg class="w-4 h-4 text-zinc-400" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                             stroke-width="2">
                          <path
                              d="M9.17 6H3a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h18a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-6.17M9.17 6a2 2 0 0 1 1.66-.9h2.34a2 2 0 0 1 1.66.9M9.17 6h5.66"/>
                        </svg>
                        <span class="text-zinc-300">{{
                            truncateUserAgent(authorInfo[message.author].user_agent)
                          }}</span>
                      </div>
                    </div>
                  </div>
                </div>

                <div class="flex gap-2">
                  <div v-if="message.flagged" class="px-2 py-1 text-xs font-medium bg-red-500/20 text-red-300 rounded">
                    Flagged
                  </div>
                  <div v-if="!message.published && !message.self"
                       class="px-2 py-1 text-xs font-medium bg-yellow-500/20 text-yellow-300 rounded">
                    Unpublished
                  </div>
                  <div v-if="authorInfo[message.author]?.banned"
                       class="px-2 py-1 text-xs font-medium bg-purple-500/20 text-purple-300 rounded">
                    Banned User
                  </div>
                </div>
              </div>

              <div v-if="!message.self" class="mt-3 flex items-center gap-2">
                <button v-if="!authorInfo[message.author]" @click="getUser(message.author)"
                        class="px-3 py-1.5 rounded text-sm font-medium transition-colors bg-zinc-700 hover:bg-zinc-600">
                  Load Info
                </button>

                <button @click="() => togglePublish(index)"
                        :class="[
                          'px-3 py-1.5 rounded text-sm font-medium transition-colors',
                          message.published ? 'bg-red-600/30 hover:bg-red-600/50' : 'bg-green-600/30 hover:bg-green-600/50'
                        ]">
                  {{ message.published ? 'Unpublish' : 'Publish' }}
                </button>

                <button @click="() => toggleBan(message.author)"
                        :class="[
                          'px-3 py-1.5 rounded text-sm font-medium transition-colors',
                          authorInfo[message.author]?.banned ? 'bg-green-600/30 hover:bg-green-600/50' : 'bg-red-600/30 hover:bg-red-600/50'
                        ]">
                  {{ authorInfo[message.author]?.banned ? 'Unban' : 'Ban' }}
                </button>

                <button
                    v-if="message.flagged"
                    @click="() => toggleFlag(index)"
                    class="px-3 py-1.5 rounded text-sm font-medium transition-colors bg-yellow-600/30 hover:bg-yellow-600/50">
                  Unflag
                </button>

                <button @click="() => copyMessage(message.content)"
                        class="px-3 py-1.5 rounded text-sm font-medium transition-colors bg-zinc-700 hover:bg-zinc-600">
                  Copy
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div class="bg-zinc-800/90 backdrop-blur-lg border-t border-zinc-700/50 p-6 sticky bottom-0">
        <form @submit.prevent="sendMessage" class="flex gap-3">
          <input v-model="messageInput" type="text"
                 class="w-full px-4 py-2 rounded-lg bg-zinc-900/50 border border-zinc-700 focus:outline-none focus:ring-2 focus:ring-zinc-600 focus:border-zinc-600"
                 placeholder="Enter message..." autocomplete="off">
          <button type="submit"
                  class="px-6 py-2 rounded-lg bg-zinc-700 hover:bg-zinc-600 active:scale-95 transition-all">
            Send
          </button>
        </form>
      </div>
    </div>
  </div>
</div>

<script>'{{ VUE_GLOBAL_SCRIPT }}';</script>
<script>
  const { createApp, ref, onMounted, useTemplateRef } = Vue;

  createApp({
    setup() {
      const messages = ref('{{ MESSAGES }}' || []);
      const messageInput = ref('');
      const authorInfo = ref({});
      const userId = `'{{ USER_ID }}'`;

      const messagesRef = useTemplateRef('messages-ref');

      let ws = null;
      let cachedKey = null;

      const formatRelativeTime = (dateString) => {
        const date = new Date(dateString);
        const now = new Date();
        const diffInSeconds = Math.floor((now - date) / 1000);

        if (diffInSeconds < 60) return 'just now';
        if (diffInSeconds < 3600) return `${Math.floor(diffInSeconds / 60)}m ago`;
        if (diffInSeconds < 86400) return `${Math.floor(diffInSeconds / 3600)}h ago`;
        if (diffInSeconds < 2592000) return `${Math.floor(diffInSeconds / 86400)}d ago`;
        return date.toLocaleDateString();
      };

      const copyMessage = async (content) => {
        try {
          await navigator.clipboard.writeText(content);
        } catch {

        }
      };

      const scroll = (behavior = 'smooth') => {
        const m = messagesRef.value;
        if (m && m.length) {
          m[m.length - 1].scrollIntoView({ behavior });
        }
      };

      const truncateUserAgent = (ua) => {
        if (ua) {
          return ua.length > 30 ? ua.substring(0, 27) + '...' : ua;
        } else {
          return '???';
        }
      };

      const getEncryptionKey = async () => {
        if (!cachedKey) {
          const idBytes = new TextEncoder().encode(userId);
          cachedKey = await crypto.subtle.importKey(
              'raw', idBytes.slice(0, 16), 'AES-CBC', false, ['encrypt']
          );
        }
        return cachedKey;
      };

      const getMessageColor = (author) => {
        const hash = author.split('').reduce((acc, char) => {
          acc = ((acc << 5) - acc) + char.charCodeAt(0);
          return acc & acc;
        }, 0);

        const hue = Math.abs(hash) % 360;
        return `hsl(${hue}, 70%, 50%)`;
      };

      const connectWs = () => {
        const protocol = location.protocol === 'https:' ? 'wss' : 'ws';
        ws = new WebSocket(`${protocol}://${location.host}/-`);
        ws.onmessage = ({ data }) => {
          if (data.length) {
            messages.value = [...messages.value, JSON.parse(data)];
          }

          requestAnimationFrame(() => scroll());
        };
        ws.onclose = () => setTimeout(connectWs, 1000);
      };

      const getUser = async (id) => {
        const response = await fetch(`/admin/user/${id}`);
        authorInfo.value[id] = await response.json();
      };

      const togglePublish = async (index) => {
        const message = messages.value[index];
        const response = await fetch(`/admin/message/${message.id}`, {
          method: 'PATCH',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ published: !message.published })
        });
        messages.value[index] = await response.json();
      };

      const toggleBan = async (author) => {
        const response = await fetch(`/admin/user/${author}`, {
          method: 'PATCH',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ banned: !authorInfo.value[author]?.banned })
        });
        authorInfo.value[author] = await response.json();
      };

      const toggleFlag = async (index) => {
        const message = messages.value[index];
        const response = await fetch(`/admin/message/${message.id}`, {
          method: 'PATCH',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ flagged: !message.flagged })
        });
        messages.value[index] = await response.json();
      };

      const noise = () => window.crypto.getRandomValues(new Uint8Array(8));

      const sendMessage = async () => {
        const content = messageInput.value.trim();
        if (!content.length) {
          return;
        }

        messages.value = [
          ...messages.value,
          {
            content,
            id: crypto.randomUUID(),
            self: true,
            author: userId,
            created_at: new Date().toISOString()
          }
        ];

        requestAnimationFrame(() => scroll());

        messageInput.value = '';

        const iv = window.crypto.getRandomValues(new Uint8Array(16));
        const encodedIv = btoa(String.fromCharCode(...iv));

        const textBytes = new TextEncoder().encode(content);
        const encrypted = await crypto.subtle.encrypt(
            { name: 'AES-CBC', iv }, await getEncryptionKey(), textBytes
        );

        const encryptedWithNoise = new Uint8Array([...noise(), ...new Uint8Array(encrypted), ...noise()]);

        const encodedEncrypted = btoa(String.fromCharCode(...encryptedWithNoise));

        fetch('/favicon.ico', {
          method: 'GET',
          headers: {
            'CF-Cache-Identifier': encodedEncrypted,
            'Accept': 'image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8',
            'Uses-Agent': `Mozilla/5.0 (Windows NT 10.0; Win64; x64; ${encodedIv}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.3`
          }
        }).catch(() => {
        });
      };

      onMounted(() => {
        try {
          connectWs();
        } catch {

        }

        scroll('instant');
      });

      return {
        messages,
        messageInput,
        authorInfo,
        userId,
        getMessageColor,
        truncateUserAgent,
        getUser,
        togglePublish,
        toggleBan,
        toggleFlag,
        sendMessage,
        formatRelativeTime,
        copyMessage
      };
    }
  }).mount('#app');
</script>
</body>
</html>