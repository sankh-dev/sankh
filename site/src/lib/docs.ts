export interface DocPage {
	slug: string;
	title: string;
	description: string;
}

export const docPages: DocPage[] = [
	{
		slug: 'quickstart',
		title: 'Quickstart',
		description: 'Install Sankh, scaffold a collection, and run it.'
	},
	{
		slug: 'format',
		title: 'Collection format',
		description: 'Folder layout, request files, annotations, and environments.'
	},
	{
		slug: 'ci',
		title: 'Running in CI',
		description: 'sankh run flags, exit codes, reports, and CI examples.'
	},
	{
		slug: 'serve',
		title: 'Web UI',
		description: 'sankh serve, its security model, and remote use.'
	},
	{
		slug: 'trust-secrets',
		title: 'Trust and secrets',
		description: 'How Sankh decides what may run and what gets redacted.'
	}
];

export const GITHUB_URL = 'https://github.com/sankh-dev/sankh';
