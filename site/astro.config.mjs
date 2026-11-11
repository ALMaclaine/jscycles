// @ts-check
import { defineConfig, passthroughImageService } from 'astro/config';
import starlight from '@astrojs/starlight';

const repository = 'https://github.com/ALMaclaine/jscycles';

export default defineConfig({
  // GitHub Pages project site, served under the account's Pages domain
  // (almaclaine.github.io redirects there).
  site: 'https://almaclaine.com',
  base: '/jscycles',
  trailingSlash: 'always',
  // The site has no raster images to optimize; skip the native sharp module.
  image: { service: passthroughImageService() },
  integrations: [
    starlight({
      title: 'jscycles',
      logo: {
        light: './src/assets/wordmark-light.svg',
        dark: './src/assets/wordmark-dark.svg',
        alt: 'jscycles',
        replacesTitle: true,
      },
      description:
        'Find circular imports in JavaScript and TypeScript projects, between files and between the packages of a monorepo.',
      social: [{ icon: 'github', label: 'GitHub', href: repository }],
      editLink: { baseUrl: `${repository}/edit/main/site/` },
      lastUpdated: false,
      customCss: ['./src/styles/custom.css'],
      sidebar: [
        {
          label: 'Guides',
          items: [
            { label: 'Install', slug: 'guides/install' },
            { label: 'Quick start', slug: 'guides/quick-start' },
            { label: 'What it finds', slug: 'guides/what-it-finds' },
            { label: 'Use in CI', slug: 'guides/ci' },
            { label: 'TypeScript and workspaces', slug: 'guides/typescript-and-workspaces' },
            { label: 'Output and JSON', slug: 'guides/output' },
            { label: 'Troubleshooting', slug: 'guides/troubleshooting' },
            { label: 'Comparison with other tools', slug: 'guides/comparison' },
          ],
        },
        {
          // Generated from docs/*.md by scripts/sync-docs.mjs.
          label: 'Reference',
          items: [
            { label: 'Command line', slug: 'reference/command-line' },
            { label: 'Configuration', slug: 'reference/configuration' },
            { label: 'Package cycles', slug: 'reference/package-cycles' },
            { label: 'Analysis limits', slug: 'reference/analysis-limits' },
            { label: 'Product contract', slug: 'reference/product-contract' },
            { label: 'Upgrading to 0.2', slug: 'reference/migration-0-2' },
          ],
        },
        { label: 'Changelog', link: `${repository}/blob/main/CHANGELOG.md` },
      ],
    }),
  ],
});
