export function formatAddress(address: string){
  return  address.substring(0, 6) + "..." + address.substring(address.length - 6)
 
}
export function capitalizeFirstLetter(string: string) {
    return string[0].toUpperCase() + string.slice(1);
}