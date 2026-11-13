// @ts-check
import { defineConfig, passthroughImageService } from 'astro/config';
import starlight from '@astrojs/starlight';

const repository = 'https://github.com/ALMaclaine/jscycles';

export default defineConfig({
  // GitHub Pages with the custom domain jscycles.dev (set in the repository's
  // Pages settings), served from the domain root.
  site: 'https://jscycles.dev',
  trailingSlash: 'always',
  // Pages that moved when the guides were reorganized.
  redirects: {
    '/guides/what-it-finds/': '/concepts/how-it-works/',
    '/guides/typescript-and-workspaces/': '/guides/typescript/',
    '/guides/output/': '/reference/json-output/',
  },
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
          label: 'Start here',
          items: [
            { label: 'Why circular imports matter', slug: 'introduction' },
            { label: 'Install', slug: 'guides/install' },
            { label: 'Quick start', slug: 'guides/quick-start' },
          ],
        },
        {
          label: 'Guides',
          items: [
            { label: 'Fixing cycles', slug: 'guides/fixing-cycles' },
            { label: 'Adopting it on an existing codebase', slug: 'guides/adopting' },
            { label: 'Use in CI', slug: 'guides/ci' },
            { label: 'Monorepos and workspaces', slug: 'guides/monorepos' },
            { label: 'TypeScript projects', slug: 'guides/typescript' },
            { label: 'Scripting with JSON', slug: 'guides/scripting' },
            { label: 'Embedding in Rust', slug: 'guides/rust-library' },
            { label: 'Migrating from madge or dpdm', slug: 'guides/migrating' },
            { label: 'Troubleshooting and FAQ', slug: 'guides/troubleshooting' },
          ],
        },
        {
          label: 'Concepts',
          items: [
            { label: 'How it works', slug: 'concepts/how-it-works' },
            { label: 'Comparison with other tools', slug: 'guides/comparison' },
          ],
        },
        {
          // All but JSON output are generated from docs/, CHANGELOG.md and
          // README.md by scripts/sync-docs.mjs.
          label: 'Reference',
          items: [
            { label: 'Command line', slug: 'reference/command-line' },
            { label: 'Configuration', slug: 'reference/configuration' },
            { label: 'JSON output', slug: 'reference/json-output' },
            { label: 'Package cycles', slug: 'reference/package-cycles' },
            { label: 'Analysis limits', slug: 'reference/analysis-limits' },
            { label: 'Product contract', slug: 'reference/product-contract' },
          ],
        },
        {
          label: 'Project',
          items: [
            { label: 'Changelog', slug: 'reference/changelog' },
            { label: 'Upgrading to 0.2', slug: 'reference/migration-0-2' },
            { label: 'Is this AI slop?', slug: 'project/is-this-ai-slop' },
            { label: 'Contributing', link: `${repository}/blob/main/CONTRIBUTING.md` },
            { label: 'Security policy', link: `${repository}/blob/main/SECURITY.md` },
          ],
        },
      ],
    }),
  ],
});
