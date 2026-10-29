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
		slug: 'install',
		title: 'Install',
		description: 'The CLI and desktop app on macOS, Linux and Windows; upgrade and uninstall.'
	},
	{
		slug: 'format',
		title: 'Collection format',
		description: 'Folder layout, request files, annotations, and environments.'
	},
	{
		slug: 'curl-flags',
		title: 'curl flags',
		description: 'Which curl options Sankh understands, ignores or rejects.'
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
		slug: 'secrets-safety',
		title: 'Secrets and safety',
		description: 'What a request file can do, what gets redacted, and limiting where a shared server sends requests.'
	},
	{
		slug: 'ai-agents',
		title: 'AI agents and MCP',
		description: 'llms.txt, the agent skill, and the sankh mcp server.'
	}
];

export const GITHUB_URL = 'https://github.com/sankh-dev/sankh';
