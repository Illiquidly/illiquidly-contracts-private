import { Address } from './terra_utils';
import { env } from './env_helper';
import { MsgExecuteContract } from '@terra-money/terra.js';

async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);
  console.log(handler.getAddress());
  // Uploading the contract code
  //let iliq_codeId: string[] = await handler.uploadContract("../artifacts/iliq_token.wasm");
  //let multisender_codeId: string[] = await handler.uploadContract("../artifacts/multisender.wasm");

  let p2p = handler.getContract(env.contracts.p2p);
  let fee = handler.getContract(env.contracts.fee);

  let response: any;

  let c = true;
  let start_after: number | undefined = undefined;
  let withdraw = [];
  let cancel_and_withdraw = [];
  let pay_fee_and_withdraw = [];

  while (c) {
    //console.log(start_after)
    response = await p2p.query.get_all_trades({
      start_after: start_after,
      limit: 20,
      filters: {
        owner: handler.getAddress()
      }
    });
    withdraw.push(
      ...response.trades.filter((trade: any) => {
        return (
          (trade.trade_info.state == 'created' ||
            trade.trade_info.state == 'cancelled') &&
          !trade.trade_info.assets_withdrawn
        );
      })
    );
    cancel_and_withdraw.push(
      ...response.trades.filter((trade: any) => {
        return trade.trade_info.state == 'published' || trade.trade_info.state == "countered";
      })
    );
    pay_fee_and_withdraw.push(
      ...response.trades.filter((trade: any) => {
        return trade.trade_info.state == 'accepted';
      })
    );
    if (response && response.trades && response.trades.length > 0) {
      start_after = response.trades[response.trades.length - 1].trade_id;
    } else {
      c = false;
    }
  }

  
  console.log("To simply withdraw", withdraw.length);
  let txArray = withdraw.map((trade) => {
    return new MsgExecuteContract(
      handler.getAddress(), // sender
      p2p.address, // contract account address
      {
        withdraw_all_from_trade: {
          trade_id: trade.trade_id
        }
      } // handle msg
    );
  });
  if (txArray.length > 0) {
    response = await p2p.execute.executeSome(txArray);
    console.log(response);
  }

	console.log("To cancel and withdraw", cancel_and_withdraw.length)
  txArray = cancel_and_withdraw.flatMap((trade) => 
    [new MsgExecuteContract(
      handler.getAddress(), // sender
      p2p.address, // contract account address
      {
        cancel_trade: {
          trade_id: trade.trade_id
        }
      } // handle msg
    ),
    new MsgExecuteContract(
      handler.getAddress(), // sender
      p2p.address, // contract account address
      {
        withdraw_all_from_trade: {
          trade_id: trade.trade_id
        }
      } // handle msg
    )
    ]
  );
  if (txArray.length > 0) {
    response = await p2p.execute.executeSome(txArray);
    console.log(response);
  }

	console.log("To withdraw accepted trades", pay_fee_and_withdraw.length)
  pay_fee_and_withdraw.map(async (trade) => {
  	console.log(trade.trade_id)
		response = await fee.query.fee({
			trade_id: trade.trade_id
		})
		console.log(response);
	}
	);
  /*




	for(let counter_trade of response.counter_trades){
		if(counter_trade.trade_info.state != "cancelled" && counter_trade.trade_info.state != "accepted")
		{
			let response = await p2p_counter.execute.cancel_counter_trade({
				trade_id: counter_trade.trade_id,
				counter_id: counter_trade.counter_id,
			})
			console.log("Cancelled Counter trade", getContractLog(response));
		}else{
			console.log("No cancel counter :/")
		}
	}


	// We query all our trades
	response = await p2p.query.get_all_trades({
		filters:{
			owner:handler.getAddress(),
		}
	})
	// We query the created trades
	
	for(let trade of response.trades){
		if(trade.trade_info.state != "cancelled" && trade.trade_info.state != "accepted")
		{
			let response = await p2p.execute.cancel_trade({
				trade_id: trade.trade_id
			})
			console.log("Cancelled trade", getContractLog(response));
		}else{
			console.log("No cancel :/")
		}
	}
	*/
}

main()
  .then((resp) => {
    console.log(resp);
  })
  .catch((err) => {
    console.log(err);
  });
