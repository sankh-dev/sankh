#!/usr/bin/env bash
# @name Deleted pet is gone
# @tags negative
# @expect status 404
# @expect json .error == "not found"
curl -sS "$BASE_URL/pets/$PET_ID" \
  -H "Authorization: Bearer $TOKEN"
