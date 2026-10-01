#!/usr/bin/env bash
# @name Create pet
# @description Creates a pet and stores its id for the next steps.
# @tags smoke
# @expect status 201
# @expect json .name == "Rex"
# @expect json .tags contains "good-boy"
# @capture PET_ID=.id
# @capture REQUEST_ID=header X-Request-Id
curl -sS "$BASE_URL/pets" \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"name":"Rex","tags":["good-boy"]}'
