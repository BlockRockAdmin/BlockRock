import { ref } from 'vue';
import mqtt from 'mqtt';
import { useAvatarStore } from '@/store';
import { useSensorStore } from '@/store/sensor';

export function useMqtt() {
  const mqttData = ref(null);
  const avatar = useAvatarStore();
  const client = mqtt.connect(import.meta.env.VITE_MQTT_BROKER);
  const sensor = useSensorStore();

  client.on('connect', () => {
    client.subscribe('blockrock/iot/light', (err) => {
      if (!err) console.log('Subscribed to light topic');
    });
    sensor.topics.forEach((t) => {
      client.subscribe(t, () => {});
    });
  });

  client.on('message', (topic, message) => {
    const data = JSON.parse(message.toString());
    mqttData.value = data;
    if (sensor.topics.includes(topic)) {
      mqttData.value = { sensor_id: topic, value: data.value };
    }
    avatar.authentications += 1;
    avatar.skills.lights = Math.min(avatar.skills.lights + 5, 100);
  });

  const publish = (topic, message) => {
    client.publish(topic, message);
  };

  return { mqttData, publish };
}
