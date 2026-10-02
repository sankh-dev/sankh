import { highlight } from '#lib/server/markdown.ts';

const requestFile = `#!/usr/bin/env bash
# @name Create pet
# @tags smoke
# @expect status 201
# @expect json .name == "Rex"
# @capture PET_ID=.id
curl -sS "$BASE_URL/pets" \\
  -H "Authorization: Bearer $TOKEN" \\
  -H 'Content-Type: application/json' \\
  -d '{"name":"Rex"}'
`;

const standalone = `set -a; . environments/dev.env; set +a
sh pets/02-create.sh`;

const layout = `my-api/
  sankh.toml
  environments/
    dev.env
    ci.env
  auth/01-login.sh
  pets/01-list.sh
  pets/02-create.sh`;

const ci = `- run: curl -fsSL https://sankh.dev/install.sh | sh
- run: >-
    sankh run . --env ci --folder smoke
    --trust --report junit
  env:
    TOKEN: \${{ secrets.API_TOKEN }}`;

const quickstart = `sankh init my-api              # scaffold a collection
sankh trust my-api             # request files are scripts: trust before running
sankh run my-api --env dev     # run everything, exit non-zero on failure
sankh serve my-api             # web UI on http://localhost:4747`;

export async function load() {
	return {
		requestFile: await highlight(requestFile, 'bash'),
		standalone: await highlight(standalone, 'bash'),
		layout: await highlight(layout, 'text'),
		ci: await highlight(ci, 'yaml'),
		quickstart: await highlight(quickstart, 'bash')
	};
}
