<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '@/stores/auth'

const router = useRouter()
const auth = useAuthStore()

const username = ref('')
const password = ref('')
const busy = ref(false)
const error = ref('')

/** 提交登录 */
async function submit(): Promise<void> {
  if (busy.value) return
  error.value = ''
  busy.value = true
  try {
    await auth.login(username.value.trim(), password.value)
    void router.replace({ name: 'projects' })
  } catch (err) {
    error.value = err instanceof Error ? err.message : '登录失败'
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <div class="login">
    <section class="login__brand">
      <h1 class="login__en">RedTeam Atlas</h1>
      <p class="login__cn">红队攻防路径拓扑平台</p>
      <p class="login__desc">基于 AI 的红队攻防路径拓扑可视化平台</p>
    </section>

    <section class="login__side">
      <div class="login__panel">
        <div class="login__mark">
          <span class="mark-box">
            <svg aria-hidden="true" viewBox="0 0 24 24"><path d="M4 18V7l6 7 3-4 5 8" /></svg>
          </span>
          <span class="mark-name">RedTeam Atlas</span>
        </div>

        <form class="login__form" @submit.prevent="submit">
        <div class="field">
          <label class="field__label" for="login-user">用户名</label>
          <input
            id="login-user"
            v-model="username"
            class="input"
            autocomplete="username"
            spellcheck="false"
            required
          />
        </div>

        <div class="field">
          <label class="field__label" for="login-pass">口令</label>
          <input
            id="login-pass"
            v-model="password"
            class="input"
            type="password"
            autocomplete="current-password"
            required
          />
        </div>

        <p v-if="error" class="login__error">{{ error }}</p>

          <button class="btn btn--primary btn--block" type="submit" :disabled="busy">
            {{ busy ? '登录中…' : '登录' }}
          </button>
        </form>
      </div>
    </section>
  </div>
</template>

<style scoped>
.login {
  min-height: 100dvh;
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  background: var(--color-paper);
}

.login__brand {
  display: grid;
  align-content: center;
  gap: var(--space-sm);
  padding: clamp(32px, 7vw, 104px);
  border-right: 1px solid var(--color-rule);
}

.login__en {
  font-size: clamp(2.2rem, 4.2vw, 3.4rem);
  line-height: 1.02;
  letter-spacing: -0.04em;
  color: var(--color-ink);
  overflow-wrap: break-word;
  min-width: 0;
}

.login__cn {
  font-size: clamp(1.05rem, 1.5vw, 1.25rem);
  color: var(--color-ink-2);
  letter-spacing: 0.01em;
}

.login__desc {
  margin-top: var(--space-2xs);
  max-width: 30ch;
  font-size: var(--text-sm);
  line-height: 1.7;
  color: var(--color-muted);
}

.login__side {
  display: grid;
  place-items: center;
  padding: var(--space-lg);
}

.login__panel {
  width: 100%;
  max-width: 300px;
  display: grid;
  gap: var(--space-lg);
}

.login__mark {
  display: flex;
  align-items: center;
  gap: var(--space-xs);
}

.mark-box {
  width: 28px;
  height: 28px;
  display: grid;
  place-items: center;
  border: 1px solid var(--color-rule-2);
  border-radius: var(--radius-pill);
  color: var(--color-accent);
}

.mark-box svg {
  width: 16px;
  height: 16px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.8;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.mark-name {
  font-size: var(--text-sm);
  letter-spacing: -0.01em;
  color: var(--color-ink);
}

.login__form {
  display: grid;
  gap: var(--space-md);
}

.login__error {
  margin-top: calc(var(--space-md) * -1 + var(--space-xs));
  font-size: var(--text-xs);
  line-height: 1.5;
  color: var(--color-bad);
}

@media (max-width: 54rem) {
  .login {
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: auto 1fr;
  }
  .login__brand {
    border-right: 0;
    border-bottom: 1px solid var(--color-rule);
    padding: var(--space-xl) var(--space-lg) var(--space-lg);
    gap: var(--space-xs);
  }
  .login__en {
    font-size: clamp(1.9rem, 8vw, 2.6rem);
  }
  .login__side {
    align-items: start;
    padding: var(--space-xl) var(--space-lg);
  }
}
</style>