#!/usr/bin/env bash
# @name Get pet
# @expect status 200
# @expect json .id | tostring == "$PET_ID"
# @expect json .status matches "^(available|pending|sold)$"
curl -sS "$BASE_URL/pets/$PET_ID" \
  -H "Authorization: Bearer $TOKEN"
