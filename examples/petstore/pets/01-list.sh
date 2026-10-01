#!/usr/bin/env bash
# @name List pets
# @tags smoke
# @expect status 200
# @expect json .items | type == "array"
curl -sS "$BASE_URL/pets" \
  -H "Authorization: Bearer $TOKEN"
