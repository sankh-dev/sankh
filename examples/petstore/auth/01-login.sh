#!/usr/bin/env bash
# @name Log in
# @tags smoke
# @expect status 200
# @expect json .token exists
# @capture TOKEN=.token
curl -sS "$BASE_URL/login" \
  -H 'Content-Type: application/json' \
  -d "{\"username\":\"$PET_USER\",\"password\":\"$PET_PASSWORD\"}"
