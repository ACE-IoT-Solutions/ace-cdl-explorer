import { readFile, writeFile } from 'node:fs/promises'

const id = process.env.GTM_CONTAINER_ID?.trim()
if (!id) {
  console.log('GTM omitted: no repository container ID configured.')
  process.exit(0)
}
if (!/^GTM-[A-Z0-9]+$/.test(id)) {
  throw new Error('GTM_CONTAINER_ID must be a GTM container ID.')
}

const indexPath = process.argv[2] ?? 'web/dist/index.html'
const html = await readFile(indexPath, 'utf8')
if (!html.includes('</head>')) throw new Error('Built index.html has no closing head tag.')
if (html.includes('<!-- ACE GTM -->')) throw new Error('GTM has already been added to this artifact.')

const snippet = `<!-- ACE GTM -->
<script>
  window.dataLayer = window.dataLayer || [];
  function gtag(){dataLayer.push(arguments);}
  gtag('js', new Date());
  window.dataLayer.push({'gtm.start': Date.now(), event: 'gtm.js'});
</script>
<script async src="https://www.googletagmanager.com/gtm.js?id=${id}"></script>
`
await writeFile(indexPath, html.replace('</head>', `${snippet}</head>`))
console.log('GTM added to the Pages artifact.')
