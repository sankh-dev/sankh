#!/usr/bin/env bash
# @name Delete pet
# @expect status 204
curl -sS -X DELETE "$BASE_URL/pets/$PET_ID" \
  -H "Authorization: Bearer $TOKEN"
