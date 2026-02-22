{{/*
Expand the name of the chart.
*/}}
{{- define "hadrian.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
We truncate at 63 chars because some Kubernetes name fields are limited to this (by the DNS naming spec).
If release name contains chart name it will be used as a full name.
*/}}
{{- define "hadrian.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "hadrian.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "hadrian.labels" -}}
helm.sh/chart: {{ include "hadrian.chart" . }}
{{ include "hadrian.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "hadrian.selectorLabels" -}}
app.kubernetes.io/name: {{ include "hadrian.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create the name of the service account to use
*/}}
{{- define "hadrian.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "hadrian.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Create the name of the ConfigMap
*/}}
{{- define "hadrian.configMapName" -}}
{{- printf "%s-config" (include "hadrian.fullname" .) }}
{{- end }}

{{/*
Create the name of the Secret
*/}}
{{- define "hadrian.secretName" -}}
{{- printf "%s-secrets" (include "hadrian.fullname" .) }}
{{- end }}

{{/*
Generate the database URL based on configuration
*/}}
{{- define "hadrian.databaseUrl" -}}
{{- if eq .Values.gateway.database.type "sqlite" }}
{{- printf "sqlite://%s" .Values.gateway.database.sqlite.path }}
{{- else if .Values.gateway.database.postgres.url }}
{{- .Values.gateway.database.postgres.url }}
{{- else }}
{{- $host := .Values.gateway.database.postgres.host }}
{{- $port := .Values.gateway.database.postgres.port | default 5432 }}
{{- $database := .Values.gateway.database.postgres.database | default "gateway" }}
{{- $username := .Values.gateway.database.postgres.username | default "gateway" }}
{{- $sslMode := .Values.gateway.database.postgres.sslMode | default "prefer" }}
{{- printf "postgres://%s:$(DATABASE_PASSWORD)@%s:%d/%s?sslmode=%s" $username $host (int $port) $database $sslMode }}
{{- end }}
{{- end }}

{{/*
Generate the Redis URL based on configuration
*/}}
{{- define "hadrian.redisUrl" -}}
{{- if .Values.gateway.cache.redis.url }}
{{- .Values.gateway.cache.redis.url }}
{{- else if .Values.gateway.cache.redis.password }}
{{- printf "redis://:$(REDIS_PASSWORD)@%s:%d" .Values.gateway.cache.redis.host (int .Values.gateway.cache.redis.port) }}
{{- else }}
{{- printf "redis://%s:%d" .Values.gateway.cache.redis.host (int .Values.gateway.cache.redis.port) }}
{{- end }}
{{- end }}

{{/*
Check if any provider secrets need to be created
*/}}
{{- define "hadrian.hasProviderSecrets" -}}
{{- $hasSecrets := false }}
{{- if and .Values.gateway.providers.openrouter.enabled .Values.gateway.providers.openrouter.apiKey (not .Values.gateway.providers.openrouter.existingSecret) }}
{{- $hasSecrets = true }}
{{- end }}
{{- if and .Values.gateway.providers.openai.enabled .Values.gateway.providers.openai.apiKey (not .Values.gateway.providers.openai.existingSecret) }}
{{- $hasSecrets = true }}
{{- end }}
{{- if and .Values.gateway.providers.anthropic.enabled .Values.gateway.providers.anthropic.apiKey (not .Values.gateway.providers.anthropic.existingSecret) }}
{{- $hasSecrets = true }}
{{- end }}
{{- $hasSecrets }}
{{- end }}

{{/*
Check if database secret needs to be created
*/}}
{{- define "hadrian.hasDatabaseSecret" -}}
{{- and (eq .Values.gateway.database.type "postgres") .Values.gateway.database.postgres.password (not .Values.gateway.database.postgres.existingSecret) }}
{{- end }}

{{/*
Check if Redis secret needs to be created
*/}}
{{- define "hadrian.hasRedisSecret" -}}
{{- and (eq .Values.gateway.cache.type "redis") .Values.gateway.cache.redis.password (not .Values.gateway.cache.redis.existingSecret) }}
{{- end }}

{{/*
Get the database password secret name
*/}}
{{- define "hadrian.databaseSecretName" -}}
{{- if .Values.gateway.database.postgres.existingSecret }}
{{- .Values.gateway.database.postgres.existingSecret }}
{{- else }}
{{- include "hadrian.secretName" . }}
{{- end }}
{{- end }}

{{/*
Get the database password secret key
*/}}
{{- define "hadrian.databaseSecretKey" -}}
{{- if .Values.gateway.database.postgres.existingSecret }}
{{- .Values.gateway.database.postgres.existingSecretKey | default "password" }}
{{- else }}
{{- "database-password" }}
{{- end }}
{{- end }}

{{/*
Get the Redis password secret name
*/}}
{{- define "hadrian.redisSecretName" -}}
{{- if .Values.gateway.cache.redis.existingSecret }}
{{- .Values.gateway.cache.redis.existingSecret }}
{{- else }}
{{- include "hadrian.secretName" . }}
{{- end }}
{{- end }}

{{/*
Get the Redis password secret key
*/}}
{{- define "hadrian.redisSecretKey" -}}
{{- if .Values.gateway.cache.redis.existingSecret }}
{{- .Values.gateway.cache.redis.existingSecretKey | default "password" }}
{{- else }}
{{- "redis-password" }}
{{- end }}
{{- end }}

{{/*
Get a provider's API key secret name
*/}}
{{- define "hadrian.providerSecretName" -}}
{{- $provider := index . 0 }}
{{- $context := index . 1 }}
{{- $providerConfig := index $context.Values.gateway.providers $provider }}
{{- if $providerConfig.existingSecret }}
{{- $providerConfig.existingSecret }}
{{- else }}
{{- include "hadrian.secretName" $context }}
{{- end }}
{{- end }}

{{/*
Get a provider's API key secret key
*/}}
{{- define "hadrian.providerSecretKey" -}}
{{- $provider := index . 0 }}
{{- $context := index . 1 }}
{{- $providerConfig := index $context.Values.gateway.providers $provider }}
{{- if $providerConfig.existingSecret }}
{{- $providerConfig.existingSecretKey | default "api-key" }}
{{- else }}
{{- printf "%s-api-key" $provider }}
{{- end }}
{{- end }}

{{/*
=============================================================================
PostgreSQL Subchart Integration
=============================================================================
*/}}

{{/*
Get the PostgreSQL host - uses subchart service if enabled, otherwise external config
*/}}
{{- define "hadrian.postgresql.host" -}}
{{- if .Values.postgresql.enabled }}
{{- printf "%s-postgresql" .Release.Name }}
{{- else }}
{{- .Values.gateway.database.postgres.host }}
{{- end }}
{{- end }}

{{/*
Get the PostgreSQL port
*/}}
{{- define "hadrian.postgresql.port" -}}
{{- if .Values.postgresql.enabled }}
{{- 5432 }}
{{- else }}
{{- .Values.gateway.database.postgres.port | default 5432 }}
{{- end }}
{{- end }}

{{/*
Get the PostgreSQL database name
*/}}
{{- define "hadrian.postgresql.database" -}}
{{- if .Values.postgresql.enabled }}
{{- .Values.postgresql.auth.database | default "gateway" }}
{{- else }}
{{- .Values.gateway.database.postgres.database | default "gateway" }}
{{- end }}
{{- end }}

{{/*
Get the PostgreSQL username
*/}}
{{- define "hadrian.postgresql.username" -}}
{{- if .Values.postgresql.enabled }}
{{- .Values.postgresql.auth.username | default "gateway" }}
{{- else }}
{{- .Values.gateway.database.postgres.username | default "gateway" }}
{{- end }}
{{- end }}

{{/*
Get the PostgreSQL secret name for password lookup
*/}}
{{- define "hadrian.postgresql.secretName" -}}
{{- if .Values.postgresql.enabled }}
  {{- if .Values.postgresql.auth.existingSecret }}
    {{- .Values.postgresql.auth.existingSecret }}
  {{- else }}
    {{- printf "%s-postgresql" .Release.Name }}
  {{- end }}
{{- else if .Values.gateway.database.postgres.existingSecret }}
  {{- .Values.gateway.database.postgres.existingSecret }}
{{- else }}
  {{- include "hadrian.secretName" . }}
{{- end }}
{{- end }}

{{/*
Get the PostgreSQL secret key for password lookup
*/}}
{{- define "hadrian.postgresql.secretKey" -}}
{{- if .Values.postgresql.enabled }}
  {{- "password" }}
{{- else if .Values.gateway.database.postgres.existingSecret }}
  {{- .Values.gateway.database.postgres.existingSecretKey | default "password" }}
{{- else }}
  {{- "database-password" }}
{{- end }}
{{- end }}

{{/*
Check if PostgreSQL password is needed (subchart enabled or external with password)
*/}}
{{- define "hadrian.postgresql.passwordRequired" -}}
{{- if .Values.postgresql.enabled }}
  {{- true }}
{{- else if and (eq .Values.gateway.database.type "postgres") (or .Values.gateway.database.postgres.password .Values.gateway.database.postgres.existingSecret) }}
  {{- true }}
{{- else }}
  {{- false }}
{{- end }}
{{- end }}

{{/*
Check if we're using PostgreSQL (either subchart or external)
*/}}
{{- define "hadrian.postgresql.enabled" -}}
{{- or .Values.postgresql.enabled (eq .Values.gateway.database.type "postgres") }}
{{- end }}

{{/*
=============================================================================
Redis Subchart Integration
=============================================================================
*/}}

{{/*
Get the Redis host - uses subchart service if enabled, otherwise external config
*/}}
{{- define "hadrian.redis.host" -}}
{{- if .Values.redis.enabled }}
{{- printf "%s-redis-master" .Release.Name }}
{{- else }}
{{- .Values.gateway.cache.redis.host }}
{{- end }}
{{- end }}

{{/*
Get the Redis port
*/}}
{{- define "hadrian.redis.port" -}}
{{- if .Values.redis.enabled }}
{{- 6379 }}
{{- else }}
{{- .Values.gateway.cache.redis.port | default 6379 }}
{{- end }}
{{- end }}

{{/*
Get the Redis secret name for password lookup
*/}}
{{- define "hadrian.redis.secretName" -}}
{{- if .Values.redis.enabled }}
  {{- if .Values.redis.auth.existingSecret }}
    {{- .Values.redis.auth.existingSecret }}
  {{- else }}
    {{- printf "%s-redis" .Release.Name }}
  {{- end }}
{{- else if .Values.gateway.cache.redis.existingSecret }}
  {{- .Values.gateway.cache.redis.existingSecret }}
{{- else }}
  {{- include "hadrian.secretName" . }}
{{- end }}
{{- end }}

{{/*
Get the Redis secret key for password lookup
*/}}
{{- define "hadrian.redis.secretKey" -}}
{{- if .Values.redis.enabled }}
  {{- "redis-password" }}
{{- else if .Values.gateway.cache.redis.existingSecret }}
  {{- .Values.gateway.cache.redis.existingSecretKey | default "password" }}
{{- else }}
  {{- "redis-password" }}
{{- end }}
{{- end }}

{{/*
Check if Redis password is needed (subchart enabled with auth, or external with password)
*/}}
{{- define "hadrian.redis.passwordRequired" -}}
{{- if and .Values.redis.enabled .Values.redis.auth.enabled }}
  {{- true }}
{{- else if and (eq .Values.gateway.cache.type "redis") (or .Values.gateway.cache.redis.password .Values.gateway.cache.redis.existingSecret) }}
  {{- true }}
{{- else }}
  {{- false }}
{{- end }}
{{- end }}

{{/*
Check if we're using Redis (either subchart or external)
*/}}
{{- define "hadrian.redis.enabled" -}}
{{- or .Values.redis.enabled (eq .Values.gateway.cache.type "redis") }}
{{- end }}
