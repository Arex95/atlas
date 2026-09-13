import { defineConfig } from 'vitepress';
import { withMermaid } from 'vitepress-plugin-mermaid';
import { arexDark, arexLight } from './theme/code';

// Diagrams are text in git, so a wrong arrow shows up in a diff like
// any other mistake. Reserved for state machines and sequences —
// trees and chains stay as plain code blocks, which already read
// fine and cost nothing.
export default withMermaid({
    // Labels as SVG text rather than embedded HTML.
    //
    // With HTML labels mermaid measures the text in a foreignObject and
    // sizes the node box from that measurement — which it takes before
    // the page's webfonts have loaded, so a multi-line label ends up in
    // a box built for a different font and the last line is clipped.
    // SVG text is measured by the renderer that draws it.
    mermaid: {
        flowchart: { htmlLabels: false, useMaxWidth: false },
        securityLevel: 'strict'
    },
    ...defineConfig({
    title: 'Atlas',

    // Mermaid pulls in dayjs, which ships CommonJS. The production
    // build handles that; the dev server does not, and fails at
    // runtime with "does not provide an export named 'default'" —
    // a blank page and a console error, not a build failure.
    // Pre-bundling both makes dev match what the build already did.
    vite: {
        optimizeDeps: { include: ['mermaid', 'dayjs'] },
        ssr: { noExternal: ['mermaid'] }
    },
    description:
        'Local-first orchestrator for working with several AI agents on one project. Sessions, real terminals, a coordination bus, workflow runs with acceptance gates, and an optional self-hosted team server.',
    lastUpdated: true,
    cleanUrls: true,

    // GitHub Pages serves this from /atlas/, matching the repository name.
    // Set DOCS_BASE=/ when a custom domain serves it from the root —
    // otherwise every asset resolves to a 404.
    base: process.env.DOCS_BASE ?? '/atlas/',

    // The default GitHub themes bring seven hues of their own, one of them a
    // second orange that competes with the brand accent. See theme/code.ts.
    markdown: {
        theme: { light: arexLight, dark: arexDark }
    },

    themeConfig: {
        nav: [
            { text: 'Guide', link: '/guide/getting-started' },
            { text: 'Concepts', link: '/concepts/why-atlas' },
            { text: 'Reference', link: '/reference/http-api' }
        ],

        sidebar: [
            {
                text: 'Guide',
                items: [
                    { text: 'Getting started', link: '/guide/getting-started' },
                    { text: 'Connecting an agent', link: '/guide/connecting-agents' },
                    { text: 'Sessions and terminals', link: '/guide/sessions-and-terminals' },
                    { text: 'Coordination', link: '/guide/coordination' },
                    { text: 'Agent memory', link: '/guide/agent-memory' },
                    { text: 'Workflow runs', link: '/guide/workflow-runs' },
                    { text: 'A shared way of working', link: '/guide/shared-way-of-working' },
                    { text: 'Connect a tracker', link: '/guide/tracker' },
                    { text: 'Team mode', link: '/guide/team-mode' },
                    { text: 'Deployment', link: '/guide/deployment' }
                ]
            },
            {
                text: 'Concepts',
                items: [
                    { text: 'Why Atlas', link: '/concepts/why-atlas' },
                    { text: 'Instructions and checks', link: '/concepts/instructions-and-checks' },
                    { text: 'Operating modes', link: '/concepts/operating-modes' },
                    { text: 'State and privacy', link: '/concepts/state-and-privacy' },
                    { text: 'Trust model', link: '/concepts/trust-model' },
                    { text: 'Sync', link: '/concepts/sync' },
                    { text: 'Portable sessions', link: '/concepts/portable-sessions' }
                ]
            },
            {
                text: 'Reference',
                items: [
                    { text: 'HTTP API', link: '/reference/http-api' },
                    { text: 'MCP tools', link: '/reference/mcp-tools' },
                    { text: 'CLI', link: '/reference/cli' },
                    { text: 'Environment variables', link: '/reference/environment' }
                ]
            }
        ],

        socialLinks: [{ icon: 'github', link: 'https://github.com/Arex95/atlas' }],

        search: { provider: 'local' },

        footer: {
            message: 'Released under the MIT License.',
            copyright: 'Built by <a href=\'https://github.com/Arex95\'>Arex95</a>'
        }
    }
    })
});
