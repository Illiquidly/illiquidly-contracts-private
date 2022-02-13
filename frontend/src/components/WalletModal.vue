<script setup lang="ts">

import { getController } from 'controller';
 interface Wallet {
  icon: string,
  name: string,
  id:string,
  present:boolean
 }
 let wallets: Wallet[] = [{
  icon:"https://assets.terra.money/icon/station-extension/icon.png",
  name:"Terra Station",
  id:"EXTENSION",
  identifier:"station",
  present:true
 },
 {
  icon:"https://assets.terra.money/icon/wallet-provider/walletconnect.svg",
  name:"Wallet Connect",
  id:"WALLETCONNECT",
  present:true
 },
 {
  icon:"https://xdefi-prod-common-ui.s3.eu-west-1.amazonaws.com/logo.svg",
  name:"Install XDEFI Wallet",
  present:false
 },
 {
  icon:"http://leapwallet.io/icon.png",
  name:"Install Leap Wallet",
  present:false,
 }];

 function connect(wallet: Wallet){
  if(!wallet || wallet.present){
    $('#walletModal').modal('hide');
    if(wallet){
      getController().connect(wallet.id, wallet.identifier);
    }else{
      getController().connect("READONLY");
    }
  }  
 }

</script>

<template>
  <div class="modal fade" id="walletModal" tabindex="-1" aria-labelledby="exampleModalLabel" aria-hidden="true">
  <div class="modal-dialog modal-dialog-centered">
    <div class="modal-content">
      <div class="modal-header">
        <h5 class="modal-title" id="exampleModalLabel">Connect your preferred Wallet</h5>
      </div>
      <div class="modal-body">
        <div v-for="wallet in wallets" 
          :key="wallet.name" 
          :class="['wallet-possibility',{'possible':wallet.present}]" 
          :title="!wallet.present ? 'Not supported yet' : ''"
          @click="connect(wallet)">
          <img class="wallet-select-icon" :src="wallet.icon" width="30" alt="Terra Station Wallet"/>
          <span class="wallet-select-text">
            {{ wallet.name }}
          </span>
        </div>
      </div>
      <div class="modal-footer">
        <p>
          <div 
            role="button" 
            class="connect-address wallet-possibility" 
            data-placement="bottom" 
            title="No transactions will be allowed" 
            data-toggle="tooltip"
            @click="connect()"
          >
            Or just use a custom address 
          </div>
        </p>
      </div>
    </div>
  </div>
</div>
</template>
<style>
  .modal.show{
    align-items: center;
  }
  .modal-title{
    color: #0222ba;
    font-size: 20px;
    font-weight: 550;
  }
  .modal-dialog{
    width: 90vw;
  }
  .modal-content{
    border: 0;
    border-radius: 10px;
    padding: 0px 10px;
  }
  .modal-footer{
    text-align: center;
    justify-content: center !important;
  }
  .wallet-possibility{
    align-items: center;
    background-color: #fff;
    border: 1px solid #0222ba;
    border-radius: 10px;
    color: #0222ba;
    display: flex;
    font-size: 18px;
    font-weight: 700;
    justify-content: center;
    padding: 20px;
    width: 100%;
    cursor: pointer;
    margin: 10px 0px;
  }
  .wallet-possibility.possible{
    background-color: #dee4ff;
    border-color: transparent!important;
  }
  .wallet-select-text{
    padding: 0px 0.8em;
    vertical-align: middle;
    line-height: 1;
  }
  .connect-address{
    cursor: pointer;
    font-weight: 100 !important;
    border: 1px solid #0222ba;
    border-radius: 25px;
    padding: 10px 25px;
  }
  .connect-address:hover{
    background-color: #dee4ff;
  }
</style>
