// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import starlightLinksValidator from 'starlight-links-validator'
import tailwind from '@astrojs/tailwind';

// https://astro.build/config
export default defineConfig({
    site: 'https://opensource.axo.dev/dist',
    integrations: [starlight({
        title: 'dist',
        logo: {
            light: './src/assets/package-light.svg',
            dark: './src/assets/package-dark.svg',
        },
        plugins: [starlightLinksValidator()],
        customCss: ['./src/styles/custom.css', './src/styles/fonts.css', './src/styles/tailwind.css'],
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
        components: {
            // Override the default `SocialIcons` component.
            SocialIcons: './src/components/overrides/SocialIcons.astro',
        },
        sidebar: [
            {
                label: 'Start here',
                items: [
                    { label: 'Installation', slug: 'start/install' },
                    { label: 'Updating', slug: 'start/update' },
                    { label: 'Project structure', slug: 'start/structure' },
                    { label: 'Configuration', slug: 'start/config' }
                ]
            },
            {
                label: 'Reference',
                autogenerate: { directory: 'reference' },
            },
        ],
        }), tailwind({
            // Disable the default base styles:
            applyBaseStyles: false,
        })
    ],
});
