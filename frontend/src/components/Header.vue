<script setup lang="ts">
  import { formatAddress, capitalizeFirstLetter } from "format";
  import WalletModal from './WalletModal.vue';
  import { getControllerAsync } from 'controller';
  import { onMounted, ref } from 'vue';
  const controller = getControllerAsync();

  const connectedWallet = ref<ConnectedWallet | undefined>(undefined);
  const networkName = ref<WalletStates | null>(null);


  onMounted(() => {

    // We make sure our Wallet is connected
    controller.then((controller) => controller.connectedWallet().subscribe(
      _connectedWallet => connectedWallet.value = _connectedWallet
    ));

    controller.then((controller) => controller.states().subscribe(
    (_states) => networkName.value = _states.network.name
    ));

  });
  function connect(event){
    console.log(!!connectedWallet.value)
    if(!!connectedWallet.value){
      event.stopPropagation();
      controller.then((controler) => controler.disconnect());
    }
  }

</script>

<template>
  <div class="header-container"> 
    <img class="header-logo" src="/banner.png" height="45"/>
    <div class="input-group trade-search ml-md-1 ml-lg-5 col-md-6 col-lg-7 d-sm-none d-md-flex">
      <input type="text" class="form-control" aria-label="Amount (to the nearest dollar)"/>
      <div class="input-group-append">
        <button class="btn btn-outline-secondary search-button" type="button"><i class="gg-search"></i></button>
      </div>
    </div>
    <div 
      class="wallet-container" 
      type="button"
      data-toggle="modal" 
      data-target="#walletModal" 
      @click="connect"
    >
      <svg class="wallet-icon" width="16px" height="16px" viewBox="0 0 16 16" version="1.1" xmlns="http://www.w3.org/2000/svg">
        <path d="M12,8 C11.4478125,8 11,8.4478125 11,9 C11,9.5521875 11.4478125,10 12,10 C12.5521875,10 13,9.5521875 13,9 C13,8.4478125 12.5521875,8 12,8 Z M14.5,3 L14,3 L14,2.5 C14,1.6715625 13.3284375,1 12.5,1 L3,1 C1.343125,1 0,2.343125 0,4 L0,12 C0,13.656875 1.343125,15 3,15 L14,15 C15.1046875,15 16,14.1046875 16,13 L16,4.5 C16,3.6715625 15.3284375,3 14.5,3 Z M15,13 C15,13.55125 14.55125,14 14,14 L3,14 C1.8971875,14 1,13.1028125 1,12 L1,4 C1,2.8971875 1.8971875,2 3,2 L12.5,2 C12.775625,2 13,2.224375 13,2.5 L13,3 L3.5,3 C3.22375,3 3,3.22375 3,3.5 C3,3.77625 3.22375,4 3.5,4 L14.5,4 C14.775625,4 15,4.224375 15,4.5 L15,13 Z" fill="currentColor" fill-rule="nonzero">
          
        </path>
      </svg>
      <span v-if="!connectedWallet" class="wallet-text">
        Connect Wallet
      </span>
      <span v-else class="wallet-text">
        {{ formatAddress(connectedWallet.terraAddress) }} &nbsp;| &nbsp;{{ capitalizeFirstLetter(networkName) }}
      </span>
    </div>
  </div>

<!-- Modal -->
<WalletModal />
</template>
<style>
  body{
    margin: 0 !important;
    padding: 0px !important;
    overflow:  auto !important;
  }
  .header-container{
    display: flex;
    background-color: #d3dfee;
    padding: 10px;
  }
  .wallet-container{
    margin-left: auto;
    margin-right: 13px;
    display: block;
    padding: 10px;
    border-radius: 15px;
    cursor: pointer;
    background-color: #88a9ce;
    transition:  background 0.5s ease;
    border: none;
  }
  .wallet-container:hover{
    background-color: #295284;
  }
  .wallet-icon{
    vertical-align: middle;
    color: white;
  }
  .wallet-text{
    font-family: Montserrat, sans-serif;
    color: #fcfffe;
    vertical-align: middle;
    padding: 10px 10px;
  }
  .trade-search{
    margin-left: 50px;
  }
  .form-control{
    border-radius: 20px;
    border: 0;
    margin-left: auto;
    background-color: white;
    height: auto
  }
  .form-control:focus{
    border: 0px;
    box-shadow: none;
  }
  .search-button{
    border: 0px;
    position: relative;
    border-radius: 20px;
    background-color: white;
    padding: 0px 15px 0px 10px;
  }
  .search-button:hover{
    background-color: white;
    color: inherit;
  }
</style>
