/** @type {import('tailwindcss').Config} */

import starlightPlugin from '@astrojs/starlight-tailwind';
import colors from 'tailwindcss/colors';

export default {
	content: ['./src/**/*.{astro,html,js,jsx,md,mdx,svelte,ts,tsx,vue}'],
	theme: {
		extend: {
            colors: {
                accent: colors.pink,
                gray: colors.zinc,
            },
            fontFamily: {
            }
        },
	},
  plugins: [starlightPlugin()],
}
