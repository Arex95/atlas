import DefaultTheme from 'vitepress/theme';
import type { Theme } from 'vitepress';

// Self-hosted rather than pulled from a font CDN. On a public site that also
// means no third-party request carrying a reader's IP on every page load.
import '@fontsource-variable/inter';
import '@fontsource-variable/space-grotesk';

import './brand.css';
import DiagramCanvas from './DiagramCanvas.vue';

export default {
    extends: DefaultTheme,
    // Registered globally so a diagram is wrapped by writing one tag in
    // markdown, with no per-page import to remember.
    enhanceApp({ app }) {
        app.component('DiagramCanvas', DiagramCanvas);
    }
} satisfies Theme;
