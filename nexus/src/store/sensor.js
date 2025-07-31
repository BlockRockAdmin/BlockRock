import { defineStore } from 'pinia';

export const useSensorStore = defineStore('sensor', {
  state: () => ({
    topics: ['sensor-1'],
  }),
});
