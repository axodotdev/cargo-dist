// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import starlightLinksValidator from 'starlight-links-validator'
import sitemap from '@astrojs/sitemap';

// https://astro.build/config
export default defineConfig({
    site: 'https://opensource.axo.dev/dist',
    integrations: [starlight({
        title: 'dist',
        plugins: [starlightLinksValidator()],
        customCss: ['./src/styles/custom.css',],
        favicon: 'public/favicon.svg',
        head: [
            // Add ICO favicon fallback for Safari.
            {
                tag: 'link',
                attrs: {
                    rel: 'icon',
                    href: '/public/favicon.ico',
                    sizes: '32x32',
                },
            },
        ],
        social: {
            github: 'https://github.com/axodotdev/cargo-dist',
            twitter: 'https://twitter.com/axodotdev',
            mastodon: 'https://mastodon.social/axodotdev',
        },
        sidebar: [
            {
                label: 'Guides',
                items: [
                    // Each item here is one entry in the navigation menu.
                    { label: 'Example Guide', slug: 'guides/example' },
                ],
            },
            {
                label: 'Reference',
                autogenerate: { directory: 'reference' },
            },
        ],
		}), sitemap()],
});
