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
		slug: 'assertions',
		title: 'Assertions and chaining',
		description: 'Check responses, capture values, and chain requests into a flow.'
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
		slug: 'desktop',
		title: 'Desktop app',
		description: 'Download, requirements, and how the desktop app runs requests.'
	},
	{
		slug: 'workspaces',
		title: 'Workspaces',
		description: 'Several collections side by side, session workspaces, and Scratch.'
	},
	{
		slug: 'import',
		title: 'Import from Postman',
		description: 'Convert a Postman collection and its environments into a Sankh folder.'
	},
	{
		slug: 'trust-secrets',
		title: 'Trust and secrets',
		description: 'How Sankh decides what may run and what gets redacted.'
	},
	{
		slug: 'ai-agents',
		title: 'AI agents and MCP',
		description: 'llms.txt, the agent skill, and the sankh mcp server.'
	}
];

export const GITHUB_URL = 'https://github.com/sankh-dev/sankh';
