import { ref, onUnmounted } from 'vue';
import { useSensorStore } from '@/store/sensor';

export function useSensors() {
  const readings = ref({});
  const sensor = useSensorStore();
  const url = (import.meta.env.VITE_ZION_CORE_URL || '') + '/sensors/stream';
  const es = new EventSource(url);

  es.onmessage = (e) => {
    const data = JSON.parse(e.data);
    if (sensor.topics.length === 0 || sensor.topics.includes(data.sensor_id)) {
      readings.value[data.sensor_id] = data.value;
    }
  };

  onUnmounted(() => {
    es.close();
  });

  return { readings };
}
