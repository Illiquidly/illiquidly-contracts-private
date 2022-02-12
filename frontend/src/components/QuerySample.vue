<script setup lang="ts">
import { Coins } from '@terra-money/terra.js';
import {
  ConnectedWallet,
  createLCDClient,
} from '@terra-money/wallet-controller';
import { getController } from 'controller';
import { Subscription } from 'rxjs';
import { onMounted, onUnmounted, ref } from 'vue';
import { getBalances } from "client";

const controller = getController();

const connectedWallet = ref<ConnectedWallet | undefined>(undefined);
const balance = ref<Coins | null>(null);

let subscription: Subscription | null = null;

onMounted(() => {

  // We make sure our Wallet is connected
  controller.connectedWallet().subscribe(
    _connectedWallet => connectedWallet.value = _connectedWallet
  );

  // We get the balance in any case
  getBalances().subscribe(b => {
    if(b) balance.value = b[0];
  });
});

onUnmounted(() => {
  subscription?.unsubscribe();
  connectedWallet.value = undefined;
});
</script>

<template>
  <h1>Query Sample</h1>
  <p v-if="!connectedWallet">Wallet not connected!</p>
  <pre v-else-if="!!balance">{{ balance.toString() }}</pre>
</template>
