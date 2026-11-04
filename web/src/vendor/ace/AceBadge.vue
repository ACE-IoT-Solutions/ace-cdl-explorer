<template>
  <span class="ace-badge" :class="`ace-badge--${tone}`">
    <slot />
  </span>
</template>

<script setup lang="ts">
withDefaults(
  defineProps<{
    tone?: 'neutral' | 'success' | 'warning' | 'danger' | 'info'
  }>(),
  {
    tone: 'neutral',
  },
)
</script>

<style scoped>
.ace-badge {
  display: inline-flex;
  align-items: center;
  min-height: 1.45rem;
  border: 1px solid var(--ace-border);
  border-radius: 999px;
  padding: 0.18rem 0.55rem;
  font-size: 0.72rem;
  font-weight: 700;
  line-height: 1;
}

.ace-badge--neutral {
  background: rgba(255, 255, 255, 0.05);
  color: var(--ace-text-muted);
}

.ace-badge--success {
  background: rgba(193, 211, 1, 0.12);
  border-color: rgba(193, 211, 1, 0.38);
  color: #E6F279;
}

.ace-badge--warning {
  background: rgba(255, 196, 77, 0.12);
  border-color: rgba(255, 196, 77, 0.36);
  color: #FFD37D;
}

.ace-badge--danger {
  background: var(--ace-danger-soft);
  border-color: rgba(246, 81, 29, 0.44);
  color: #FFB5A0;
}

.ace-badge--info {
  background: var(--ace-secondary-soft);
  border-color: rgba(173, 252, 249, 0.4);
  color: var(--ace-secondary);
}
</style>
