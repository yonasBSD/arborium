<template>
  <div class="counter">
    <h1>{{ title }}</h1>
    <p>Count: {{ count }}</p>
    <button @click="increment" :disabled="count >= max">
      Increment
    </button>
    <ul>
      <li v-for="item in items" :key="item.id">
        {{ item.name }}
      </li>
    </ul>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';

interface Item {
  id: number;
  name: string;
}

const props = defineProps<{ max: number }>();
const count = ref(0);
const items = ref<Item[]>([]);

const title = computed(() => `Counter (max: ${props.max})`);

function increment() {
  if (count.value < props.max) count.value++;
}

onMounted(() => {
  items.value = [{ id: 1, name: 'Apple' }, { id: 2, name: 'Banana' }];
});
</script>

<style scoped>
.counter { padding: 1rem; }
button { background: #42b883; }
</style>
