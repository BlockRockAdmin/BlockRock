<template>
  <Dashboard>
    <v-container fluid>
      <v-row>
        <v-col cols="12" sm="6" md="4"><MeteoWidget /></v-col>
        <v-col cols="12" sm="6" md="4"><NewsWidget /></v-col>
        <v-col cols="12" sm="6" md="4"><WalletWidget /></v-col>
        <v-col cols="12" sm="6" md="4"><NasWidget /></v-col>
        <v-col cols="12" sm="6" md="4"><MqttControl /></v-col>
        <v-col cols="12" sm="6" md="4"><SensorWidget /></v-col>
        <v-col cols="12" sm="6" md="4"><AvatarCard /></v-col>
        <v-col cols="12"><EventsLog /></v-col>
      </v-row>
    </v-container>
  </Dashboard>
</template>
<script>
import Dashboard from '@/layouts/Dashboard.vue';
import MeteoWidget from '@/components/MeteoWidget.vue';
import NewsWidget from '@/components/NewsWidget.vue';
import WalletWidget from '@/components/WalletWidget.vue';
import NasWidget from '@/components/NasWidget.vue';
import MqttControl from '@/components/MqttControl.vue';
import SensorWidget from '@/components/SensorWidget.vue';
import AvatarCard from '@/components/AvatarCard.vue';
import EventsLog from '@/components/EventsLog.vue';

export default {
  components: { Dashboard, MeteoWidget, NewsWidget, WalletWidget, NasWidget, MqttControl, SensorWidget, AvatarCard, EventsLog },
};
</script>
