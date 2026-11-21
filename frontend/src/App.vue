<script setup lang="ts">
import ConfirmDialog from './components/ConfirmDialog.vue'
import ToastStack from './components/ToastStack.vue'
</script>

<template>
  <router-view />
  <ToastStack />
  <ConfirmDialog />
</template>