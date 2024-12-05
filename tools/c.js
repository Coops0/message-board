function generateDomains() {
    const prefixes = ['mcps', 'study', 'learn', 'edu', 'school', 'student', 'teach', 'class'];
    const suffixes = ['hub', 'space', 'net', 'lab', 'port', 'zone', 'app'];
    const tlds = ['org', 'com', 'net', 'io'];
    const patterns = [
        // simple: prefix + tld
        ...prefixes.flatMap(p => tlds.map(t => `${p}.${t}`)),
        // prefix + suffix + tld
        ...prefixes.flatMap(p =>
            suffixes.flatMap(s =>
                tlds.map(t => `${p}${s}.${t}`)
            )
        ),
        // Numbers in domain
        ...prefixes.flatMap(p =>
            ['1', '2', '3', '4'].flatMap(n =>
                tlds.map(t => `${p}${n}.${t}`)
            )
        ),
        // Hyphens
        ...prefixes.flatMap(p =>
            suffixes.flatMap(s =>
                tlds.map(t => `${p}-${s}.${t}`)
            )
        )
    ];

    // Add more targeted variations based on school keywords
    const schoolTerms = ['montgomery', 'moco', 'md', 'maryland'];
    patterns.push(
        ...schoolTerms.flatMap(t =>
            suffixes.map(s => `${t}${s}.org`)
        )
    );

    return [...new Set(patterns)];
}

async function checkDomain(domain) {
    try {
        const response = await fetch(`http://${domain}`);
        const text = await response.text();
        return { domain, status: text.includes('This website is currently unavailable') ? 'blocked' : 'accessible' };
    } catch (e) {
        if (e.cause?.code === 'ENOTFOUND') {
            return { domain, status: 'unregistered' };
        }
        return { domain, status: `error-${e.cause?.code || 'unknown'}` };
    }
}

async function scanDomains() {
    const domains = generateDomains();
    const results = [];

    for (let i = 0; i < domains.length; i += 3) {
        const batch = domains.slice(i, i + 3);
        const batchResults = await Promise.all(batch.map(checkDomain));
        results.push(...batchResults.filter(r => r.status === 'unregistered'));

        // Progress indicator
        if (i % 30 === 0) {
            console.log(`Checked ${i}/${domains.length} domains...`);
        }

        await new Promise(resolve => setTimeout(resolve, 1500));
    }

    return results;
}

scanDomains().then(results => {
    console.log('\nPotential unregistered domains:');
    results.forEach(r => console.log(r.domain));
});