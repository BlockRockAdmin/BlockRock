<template>
  <Dashboard>
    <v-container fluid>
      <v-row>
        <v-col cols="12">
          <v-card>
            <v-card-title>Impostazioni</v-card-title>
            <v-card-text>
              <v-text-field label="Sensor Topics" v-model="topics" />
              <v-btn class="mt-2" @click="save">Salva</v-btn>
            </v-card-text>
          </v-card>
        </v-col>
      </v-row>
    </v-container>
  </Dashboard>
</template>
<script>
import Dashboard from '@/layouts/Dashboard.vue';
import { useSensorStore } from '@/store/sensor';
import { ref } from 'vue';

export default {
  components: { Dashboard },
  setup() {
    const sensor = useSensorStore();
    const topics = ref(sensor.topics.join(','));
    const save = () => {
      sensor.topics = topics.value.split(',').map(t => t.trim()).filter(Boolean);
    };
    return { topics, save };
  },
};
</script>
