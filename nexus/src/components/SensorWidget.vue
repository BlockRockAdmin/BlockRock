<template>
  <v-card>
    <v-card-title>Sensori</v-card-title>
    <v-card-text>
      <div v-for="(value, id) in readings" :key="id">
        {{ id }}: {{ value }}
      </div>
    </v-card-text>
  </v-card>
</template>
<script>
import { useSensors } from '@/composables/useSensors';
export default {
  setup() {
    const { readings } = useSensors();
    return { readings };
  },
};
</script>
