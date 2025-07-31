import { ref } from 'vue';
import { useAvatarStore } from '@/store';

export function useTron() {
  const tronWeb = ref(null);
  const balance = ref(0);
  const avatar = useAvatarStore();

  const initTron = async () => {
    if (window.tronLink) {
      await window.tronLink.request({ method: 'tron_requestAccounts' });
      tronWeb.value = window.tronLink.tronWeb;
      balance.value = await tronWeb.value.trx.getBalance(tronWeb.value.defaultAddress.base58) / 1000000;
    }
  };

  const createTransaction = async (description, amount) => {
    const tx = await tronWeb.value.transactionBuilder.sendTrx(
      'DESTINATION_ADDRESS', // Sostituisci
      amount * 1000000,
      tronWeb.value.defaultAddress.base58
    );
    const signedTx = await tronWeb.value.trx.sign(tx);
    await tronWeb.value.trx.sendRawTransaction(signedTx);
    avatar.authentications += 1;
    avatar.skills.eeg = Math.min(avatar.skills.eeg + 2, 100);
    return signedTx;
  };

  initTron();

  return { balance, createTransaction };
}
