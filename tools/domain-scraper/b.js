const { promises: fs } = require('fs');

async function checkDomain(domain) {
    try {
        const response = await fetch(`http://${domain}`);
        const text = await response.text();
        return { domain, status: text.includes('This website is currently unavailable') ? 'blocked' : 'accessible' };
    } catch (e) {
        // Check if domain doesn't exist
        if (e.cause?.code === 'ENOTFOUND') {
            return { domain, status: 'unregistered' };
        }
        // Handle other specific errors
        if (e.cause?.code === 'UND_ERR_CONNECT_TIMEOUT') {
            return { domain, status: 'timeout' };
        }
        if (e.cause?.code === 'ECONNRESET') {
            return { domain, status: 'connection-reset' };
        }
        return { domain, status: `error-${e.cause?.code || 'unknown'}` };
    }
}

async function extractDomains() {
    const files = ['1.json', '2.json', '3.json', '4.json', '5.json'];
    const domains = new Set();
    const skipDomains = [
        'google', 'github', 'microsoft', 'apple', 'adobe', 'amazon',
        'amazonaws', 'cloudfront', 'widencdn', 'cdn', 'netlify',
        'herokuapp', 'azurewebsites', 'gov'
    ];

    for (const file of files) {
        const content = await fs.readFile(file, { encoding: 'utf8' });
        const data = JSON.parse(content);
        const urlStrings = JSON.stringify(data).match(/https?:\/\/[^\s"')]+/g) || [];

        for (const url of urlStrings) {
            try {
                let domain = new URL(url).hostname;
                domain = domain.replace(/^www\./, '');
                const parts = domain.split('.');
                if (parts.length > 2) {
                    domain = parts.slice(-2).join('.');
                }

                if (!skipDomains.some(skip => domain.includes(skip))) {
                    domains.add(domain);
                }
            } catch (e) {}
        }
    }

    return Array.from(domains);
}

async function findDomains() {
    const domains = await extractDomains();
    const results = [];

    for (let i = 0; i < domains.length; i += 3) {
        const batch = domains.slice(i, i + 3);
        const batchResults = await Promise.all(batch.map(checkDomain));
        results.push(...batchResults);
        await new Promise(resolve => setTimeout(resolve, 1500));
    }

    return results;
}

findDomains().then(results => {
    const grouped = {};
    results.forEach(r => {
        grouped[r.status] = grouped[r.status] || [];
        grouped[r.status].push(r.domain);
    });

    console.log('Results by status:', grouped);
    console.log('\nLikely unregistered domains:', grouped.unregistered || []);
});