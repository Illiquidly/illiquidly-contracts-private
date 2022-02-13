import { createApp } from 'vue';
import App from './App.vue';

    $(function () {
    	// @ts-ignore
      $('.wallet-possibility').tooltip({
         container: 'body',
      }); 
      // @ts-ignore
      $('[data-toggle="tooltip"]').tooltip({
         container: 'body'
      }); 
      // @ts-ignore
      $('[data-toggle="popover"]').popover({
          container: 'body'
        });
    })

createApp(App)
.mount('#app');
