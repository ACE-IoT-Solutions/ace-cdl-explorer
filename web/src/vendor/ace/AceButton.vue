<template>
  <component
    :is="href ? 'a' : 'button'"
    class="ace-button"
    :class="[`ace-button--${variant}`, `ace-button--${size}`, { 'ace-button--block': block }]"
    :href="href || undefined"
    :type="href ? undefined : type"
    :disabled="href ? undefined : disabled || loading"
    :aria-disabled="href && (disabled || loading) ? 'true' : undefined"
  >
    <span v-if="loading" class="ace-button__spinner" aria-hidden="true" />
    <slot name="icon" />
    <span><slot /></span>
  </component>
</template>

<script setup lang="ts">
withDefaults(
  defineProps<{
    variant?: 'primary' | 'secondary' | 'danger' | 'ghost' | 'quiet' | 'link'
    size?: 'sm' | 'md' | 'lg'
    type?: 'button' | 'submit' | 'reset'
    disabled?: boolean
    loading?: boolean
    block?: boolean
    href?: string
  }>(),
  {
    variant: 'primary',
    size: 'md',
    type: 'button',
    disabled: false,
    loading: false,
    block: false,
    href: '',
  },
)
</script>

<style scoped>
.ace-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
  border: 1px solid transparent;
  border-radius: var(--ace-radius-md);
  font-weight: 700;
  line-height: 1;
  text-decoration: none;
  transition: background-color 140ms ease, border-color 140ms ease, color 140ms ease, box-shadow 140ms ease;
  cursor: pointer;
}

.ace-button:focus-visible {
  outline: 0;
  box-shadow: 0 0 0 3px var(--ace-focus);
}

.ace-button:disabled,
.ace-button[aria-disabled='true'] {
  cursor: not-allowed;
  opacity: 0.55;
  pointer-events: none;
}

.ace-button--block {
  width: 100%;
}

.ace-button--sm {
  min-height: 1.9rem;
  padding: 0.4rem 0.65rem;
  font-size: 0.78rem;
}

.ace-button--md {
  min-height: 2.35rem;
  padding: 0.58rem 0.95rem;
  font-size: 0.86rem;
}

.ace-button--lg {
  min-height: 2.85rem;
  padding: 0.74rem 1.15rem;
  font-size: 0.96rem;
}

.ace-button--primary {
  background: var(--ace-primary);
  color: #15170C;
}

.ace-button--primary:hover {
  background: var(--ace-primary-hover);
}

.ace-button--secondary {
  background: var(--ace-secondary-soft);
  border-color: rgba(173, 252, 249, 0.42);
  color: var(--ace-secondary);
}

.ace-button--secondary:hover {
  background: rgba(173, 252, 249, 0.18);
}

.ace-button--danger {
  background: var(--ace-danger-soft);
  border-color: rgba(246, 81, 29, 0.52);
  color: #FFB5A0;
}

.ace-button--danger:hover {
  background: rgba(246, 81, 29, 0.2);
}

.ace-button--ghost {
  background: transparent;
  border-color: var(--ace-border);
  color: var(--ace-text);
}

.ace-button--ghost:hover {
  border-color: var(--ace-border-strong);
  background: rgba(255, 255, 255, 0.05);
}

.ace-button--quiet,
.ace-button--link {
  background: transparent;
  color: var(--ace-text-muted);
}

.ace-button--quiet:hover,
.ace-button--link:hover {
  color: var(--ace-text);
}

.ace-button--link {
  min-height: auto;
  padding: 0;
  text-decoration: underline;
  text-underline-offset: 3px;
}

.ace-button__spinner {
  width: 1em;
  height: 1em;
  border: 2px solid currentColor;
  border-right-color: transparent;
  border-radius: 50%;
  animation: ace-spin 700ms linear infinite;
}

@keyframes ace-spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
